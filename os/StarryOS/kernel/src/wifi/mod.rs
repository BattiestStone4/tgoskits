use alloc::sync::Arc;

use aic8800::common::ChipVariant;
use aic8800::fdrv::{WifiClient, init as fdrv_init, sdio1_irq_handler};
use aic8800::fw;
use sdhci_cv1800::{
    CviSdhci,
    hw_init::{Sdio1HwConfig, sdio1_hw_init},
};
use sdio_host::SdioHost;

fn sdio1_irq_trampoline(_irq: usize) {
    sdhci_cv1800::irq::sdhci_irq_handler(0);
}

pub fn probe_wifi() {
    let pvo = ax_config::plat::PHYS_VIRT_OFFSET;

    let cfg = Sdio1HwConfig::new(
        ax_config::devices::CRG_PADDR,
        ax_config::devices::SYSCON_PADDR,
        ax_config::devices::RTCSYS_CTRL_PADDR,
        ax_config::devices::RTCSYS_IO_PADDR,
        ax_config::devices::SDIO1_PADDR,
        pvo,
    );

    info!("[wifi] SDIO1 HW init starting...");
    sdio1_hw_init(&cfg);
    info!("[wifi] SDIO1 HW init done");

    // Register SDIO1 PLIC IRQ
    let irq = ax_config::devices::WIFI_IRQ as usize;
    if !ax_hal::irq::register(irq, sdio1_irq_trampoline) {
        error!("[wifi] Failed to register SDIO1 IRQ {}", irq);
        return;
    }

    // SDHCI init
    let mut sdio = CviSdhci::new(cfg.sdio1_base_va);
    if let Err(e) = sdio.init() {
        error!("[wifi] SDIO1 init failed: {:?}", e);
        return;
    }

    let (vid, did) = sdio.vendor_device_id();
    let chip = ChipVariant::from_vid_did(vid, did);
    info!(
        "[wifi] AIC8800 detected: vendor=0x{:04x}, device=0x{:04x}, chip={:?}",
        vid, did, chip
    );

    if chip == ChipVariant::Unknown {
        error!("[wifi] Unknown WiFi chip");
        return;
    }

    // Prepare SDHCI for first data transfer (clear stale DAT state)
    sdio.prepare_first_data_xfer();

    // Firmware download
    info!("[wifi] Downloading firmware...");
    if let Err(e) = fw::firmware_init(&mut sdio, chip) {
        error!("[wifi] Firmware init failed: {:?}", e);
        return;
    }
    info!("[wifi] Firmware download complete");

    // FDRV init
    info!("[wifi] FDRV init starting...");
    let bus = match fdrv_init(sdio, chip) {
        Ok(bus) => bus,
        Err(e) => {
            error!("[wifi] FDRV init failed: {}", e);
            return;
        }
    };
    info!("[wifi] FDRV init complete");

    // Register CARD_INT callback
    sdhci_cv1800::irq::register_card_irq_callback(sdio1_irq_handler);

    // ================================================================
    // Start a softAP (open network).
    //
    // Runs the full vendor SDIO sequence: base LMAC config, add an AP-type
    // interface, then beacon download (APM_SET_BEACON_IE_REQ) followed by
    // APM_START_REQ with real beacon metadata. A valid APM_START_CFM with
    // status==0 means the AP is up and broadcasting.
    // ================================================================
    let mut client = WifiClient::new(Arc::clone(&bus));
    let channel = 6u8;
    let ssid = b"PicoClaw-Car";
    match client.start_ap_open(chip, ssid, channel, 6000) {
        Ok(cfm) => {
            info!("==========================================================");
            info!("[wifi] AP started! APM_START_CFM={:02x?}", cfm);
            info!("[wifi] SSID=PicoClaw-Car channel={} (open network)", channel);
            info!("==========================================================");
        }
        Err(e) => {
            error!("==========================================================");
            error!("[wifi] AP START FAILED: {:?}", e);
            error!("[wifi] STA mode still works.");
            error!("==========================================================");
        }
    }
}
