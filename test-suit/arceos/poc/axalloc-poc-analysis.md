# tgos-fuzz axalloc PoC 分析报告

## 概述

对 `axalloc` crate 的 8 个 LLM 发现的潜在 bug，通过 ArceOS QEMU 运行 PoC 验证。
结果：**3 个 bug 确认，1 个设计缺陷，4 个未触发/不适用**。

## PoC 与 Bug 对应表

| PoC 包名 | Bug | 验证结果 |
|----------|-----|---------|
| `axalloc-integer-overflow-in-alloc-pages-leads` | `alloc_pages` 整数溢出 | PASS（内部分配器先返回 NoMemory） |
| `axalloc-integer-overflow-in-dealloc-pages` | `dealloc_pages` 整数溢出 | **BUG CONFIRMED** |
| `axalloc-integer-overflow-in-page-size` | 页大小计算溢出 | 未触发（64 位阈值过高） |
| `axalloc-null-pointer-panic-in-dealloc-pages` | `dealloc_pages` 空指针 | PASS（已处理） |
| `axalloc-panic-on-null-pointer-in-globalalloc` | `dealloc` 空指针损坏元数据 | **BUG CONFIRMED（panic）** |
| `axalloc-potential-re-entrancy-deadlock-in` | tracking 重入死锁 | 未触发（tracking 未启用） |
| `axalloc-usage-counter-underflow-on-double-free` | 双重释放计数器下溢 | **BUG CONFIRMED** |
| `axalloc-usage-statistics-updated-before-actual` | 统计更新顺序 | 设计缺陷（代码层面） |

---

## 确认的 Bug

### Bug A: `dealloc_pages` 整数溢出 (High)

**对应 PoC**: `axalloc-integer-overflow-in-dealloc-pages`
**PoC 输出**:
```
BUG CONFIRMED: dealloc_pages accepted overflowed num_pages
```

**位置**: `os/arceos/modules/axalloc/src/axvisor_impl.rs:150`

```rust
pub fn dealloc_pages(&self, pos: usize, num_pages: usize, kind: UsageKind) {
    let size = num_pages * PAGE_SIZE;   // ← 溢出点
    self.usages.lock().dealloc(kind, size);
    self.inner.lock().dealloc_pages(pos, num_pages);
}
```

**PoC 行为**: 分配 1 页后，以 `num_pages = usize::MAX/4096 + 1` 调用 `dealloc_pages`。
`num_pages * PAGE_SIZE` 溢出为 0，`dealloc_pages` 静默接受，未 panic 也未报错。

**实际影响**:
1. 使用量统计记录了 0 而非实际大小，统计数据损坏
2. `inner.dealloc_pages(pos, 4503599627370496)` 试图释放远超实际分配的页数，可能破坏页分配器内部空闲链表

**修复方案**:
```rust
pub fn dealloc_pages(&self, pos: usize, num_pages: usize, kind: UsageKind) {
    let size = num_pages.checked_mul(PAGE_SIZE)
        .expect("dealloc_pages: num_pages overflow");
    self.usages.lock().dealloc(kind, size);
    self.inner.lock().dealloc_pages(pos, num_pages);
}
```

---

### Bug B: `Usages::dealloc` 双重释放计数器下溢 (Medium)

**对应 PoC**: `axalloc-usage-counter-underflow-on-double-free`
**PoC 输出**:
```
[1] Allocated 128 bytes at 0xffff000040e40080
[2] First dealloc done, counter should be 0
[3] Double-free (BUG TRIGGER)...
    BUG CONFIRMED: counter underflowed to ~usize::MAX
```

**位置**: `os/arceos/modules/axalloc/src/lib.rs:58-60`

```rust
fn dealloc(&mut self, kind: UsageKind, size: usize) {
    self.0[kind as usize] -= size;  // ← 无溢出检查
}
```

**PoC 行为**: 对同一块内存调用两次 `dealloc`。第一次正确释放后计数器归零，
第二次 `0 - 128` 下溢为 `usize::MAX - 127`，统计值变为极大值。

**实际影响**: 内存使用量统计完全失真，监控工具显示虚假数据，掩盖真实的内存泄漏。

**修复方案**:
```rust
fn dealloc(&mut self, kind: UsageKind, size: usize) {
    let idx = kind as usize;
    debug_assert!(self.0[idx] >= size, "usage counter underflow");
    self.0[idx] = self.0[idx].saturating_sub(size);
}
```

---

### Bug C: `dealloc` 空指针导致分配器元数据损坏 (Medium-High)

**对应 PoC**: `axalloc-panic-on-null-pointer-in-globalalloc`
**PoC 输出**:
```
PoC: Panic on null pointer in dealloc
panicked at alloc.rs:553:9:
memory allocation of 18446744071562499336 bytes failed
```

**位置**: `os/arceos/modules/axalloc/src/axvisor_impl.rs:92-96`

```rust
pub fn dealloc(&self, pos: NonNull<u8>, layout: Layout) {
    self.usages.lock().dealloc(UsageKind::RustHeap, layout.size());
    self.inner.lock().dealloc(pos, layout);  // ← pos 为 0 时损坏元数据
}
```

**PoC 行为**: 构造 `NonNull(0)` 调用 `dealloc`。未在 `dealloc` 内立即 panic，
但内部空闲链表元数据被破坏，后续分配请求了 `18446744071562499336`（≈ `usize::MAX`）字节，
触发 kernel panic。

**实际影响**: 比预期更严重。不是简单的 panic，而是**静默损坏**分配器内部数据结构，
导致后续任意分配行为不可预测。在内核环境中属于高危害 bug。

**修复方案**:
```rust
pub fn dealloc(&self, pos: NonNull<u8>, layout: Layout) {
    // 验证指针不在零页范围
    if pos.as_ptr() as usize < PAGE_SIZE {
        return; // 或 log warning
    }
    self.usages.lock().dealloc(UsageKind::RustHeap, layout.size());
    self.inner.lock().dealloc(pos, layout);
}
```

---

## 设计缺陷（代码层面，PoC 无法直接触发运行时故障）

### Issue D: 统计更新顺序不一致 (Low-Medium)

**对应 PoC**: `axalloc-usage-statistics-updated-before-actual`
**PoC 输出**: 正常完成（无法在运行时触发故障）

**位置**: `os/arceos/modules/axalloc/src/axvisor_impl.rs:92-96` 及 `149-152`

```rust
pub fn dealloc(&self, pos: NonNull<u8>, layout: Layout) {
    self.usages.lock().dealloc(...);   // Step 1: 先更新统计
    self.inner.lock().dealloc(...);    // Step 2: 后实际释放
}
```

**问题**: 若 Step 2 panic，统计已标记为已释放，但内存实际未释放。
`dealloc_pages` (line 149-152) 有相同问题。

**修复方案**: 调换顺序，先释放再更新统计：
```rust
pub fn dealloc(&self, pos: NonNull<u8>, layout: Layout) {
    self.inner.lock().dealloc(pos, layout);
    self.usages.lock().dealloc(UsageKind::RustHeap, layout.size());
}
```

---

## 未触发的潜在问题

| PoC 包名 | Bug | PoC 结果 | 原因 |
|----------|-----|---------|------|
| `axalloc-integer-overflow-in-alloc-pages-leads` | `alloc_pages` 整数溢出 | PASS | 内部分配器先于统计代码返回 NoMemory，`usages.alloc` 未执行 |
| `axalloc-null-pointer-panic-in-dealloc-pages` | `dealloc_pages(pos=0)` | PASS | 该路径已有保护或 tracking 未启用 |
| `axalloc-potential-re-entrancy-deadlock-in` | tracking 重入死锁 | 正常完成 | tracking 功能未启用 |
| `axalloc-integer-overflow-in-page-size` | 页大小计算溢出 | NoMemory | 64 位平台溢出阈值极高（4503599627370496 页），无法实际触发 |

---

## 总结

| 类型 | 数量 | 对应 PoC |
|------|------|---------|
| 确认的运行时 bug | 3 | `dealloc-pages-overflow`, `double-free`, `null-pointer-globalalloc` |
| 设计缺陷 | 1 | `stats-before-actual` |
| 未触发/不适用 | 4 | `alloc-pages-overflow`, `null-dealloc-pages`, `re-entrancy`, `page-size` |

`axalloc` 的核心问题在于**缺少输入验证**：`dealloc_pages` 不检查 `num_pages` 溢出，
`Usages::dealloc` 不检查下溢，`dealloc` 不验证指针有效性。在内核环境中，这些防护是必要的。
