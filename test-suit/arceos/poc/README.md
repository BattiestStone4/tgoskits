# tgos-fuzz PoC 测试用例

本目录包含由 [tgos-fuzz](https://github.com/BattiestStone4/tgos-fuzz) 自动生成的 PoC（Proof of Concept）测试用例，用于验证 LLM 静态审计发现的潜在 bug。

## 来源

这些 PoC 基于 `axalloc` 模块的静态审计报告生成。审计由 tgos-fuzz 通过 LLM 完成，PoC 作为 ArceOS 应用在 QEMU 中运行。

## 验证结果

| PoC | Bug 描述 | 严重度 | 结果 |
|-----|---------|--------|------|
| `axalloc-integer-overflow-in-alloc-pages-leads` | `alloc_pages` 整数溢出 | high | PASS（内部分配器先返回 NoMemory） |
| `axalloc-integer-overflow-in-dealloc-pages` | `dealloc_pages` 整数溢出 | high | **BUG CONFIRMED** |
| `axalloc-integer-overflow-in-page-size` | 页大小计算溢出 | low | 未触发（64 位阈值过高） |
| `axalloc-null-pointer-panic-in-dealloc-pages` | `dealloc_pages` 空指针 | medium | PASS（已处理） |
| `axalloc-panic-on-null-pointer-in-globalalloc` | `dealloc` 空指针损坏元数据 | medium | **BUG CONFIRMED** |
| `axalloc-potential-re-entrancy-deadlock-in` | tracking 重入死锁 | high | 未触发（tracking 未启用） |
| `axalloc-usage-counter-underflow-on-double-free` | 双重释放计数器下溢 | medium | **BUG CONFIRMED** |
| `axalloc-usage-statistics-updated-before-actual` | 统计更新顺序 | medium | 设计缺陷 |

详细分析见 tgos-fuzz 输出的 `axalloc-poc-analysis.md`。

## 运行方式

```bash
# 在 tgoskits 根目录下运行（需要 Linux 环境 + QEMU）
cargo xtask arceos qemu --package <poc-name> --arch riscv64

# 示例
cargo xtask arceos qemu --package arceos-axalloc-integer-overflow-in-dealloc-pages --arch riscv64
```

## 注意事项

- 这些 PoC 由 LLM 生成，经过人工修正 API 签名后通过编译
- 每个目录包含独立的 `Cargo.toml` + `src/main.rs`，作为 ArceOS workspace member 运行
- 此分支（`poc`）仅用于展示 PoC 测试用例，不建议合入主线
