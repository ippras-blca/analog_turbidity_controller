use anyhow::Result;
use esp_idf_svc::{
    hal::{
        adc::{
            attenuation::DB_11,
            oneshot::{AdcChannelDriver, AdcDriver, config::AdcChannelConfig},
        },
        gpio::ADCPin,
        peripheral::Peripheral,
    },
    sys::EspError,
};
use log::{info, trace};
use tokio::{
    spawn,
    sync::{
        mpsc::{self, Sender},
        oneshot::Sender as OneshotSender,
    },
};

pub(crate) type Request = OneshotSender<Result<u16, EspError>>;

// https://www.reddit.com/r/esp32/comments/1b6fles/adc2_is_no_longer_supported_please_use_adc1
pub(super) fn start<T: ADCPin>(
    adc: impl Peripheral<P = <T as ADCPin>::Adc> + 'static,
    pin: impl Peripheral<P = T> + 'static,
) -> Result<Sender<Request>> {
    info!("Start turbidity reader");
    // Turbidimeter driver
    let adc = AdcDriver::new(adc)?;
    let config = AdcChannelConfig {
        attenuation: DB_11,
        ..Default::default()
    };
    let mut driver = AdcChannelDriver::new(adc, pin, &config)?;
    info!("Turbidimeter driver initialized");
    let (sender, mut receiver) = mpsc::channel::<Request>(9);
    info!("Spawn turbidity reader");
    spawn(async move {
        while let Some(sender) = receiver.recv().await {
            let turbidity = driver.read();
            trace!("Send turbidity: {turbidity:?}");
            sender.send(turbidity).expect("Send turbidity");
        }
    });
    Ok(sender)
}
