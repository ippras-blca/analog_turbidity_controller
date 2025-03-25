use esp_idf_svc::hal::{gpio::IOPin, onewire::OWAddress, peripheral::Peripheral, rmt::RmtChannel};
use log::{info, trace};
use std::{collections::BTreeMap, ops::Range};
use thermometer::{
    Ds18b20Driver,
    scratchpad::{ConfigurationRegister, Resolution, Scratchpad},
};
use thiserror::Error;
use tokio::{
    spawn,
    sync::{
        mpsc::{self, Sender},
        oneshot::Sender as OneshotSender,
    },
};
use tokio_modbus::prelude::ExceptionCode;

pub(crate) type Request = (
    Range<usize>,
    OneshotSender<Result<BTreeMap<u64, f32>, Error>>,
);

pub(super) fn run(
    pin: impl Peripheral<P = impl IOPin> + 'static,
    channel: impl Peripheral<P = impl RmtChannel> + 'static,
) -> Result<Sender<Request>> {
    info!("Initialize temperature reader");
    let mut driver = Ds18b20Driver::new(pin, channel)?;
    info!("Temperature driver initialized");
    let mut addresses = driver.search()?.collect::<Result<Vec<OWAddress>, _>>()?;
    addresses.sort_by_key(OWAddress::address);
    for address in &addresses {
        let scratchpad = driver
            .initialization()?
            .match_rom(address)?
            .read_scratchpad()?;
        info!("{address:x?}: {scratchpad:?}");
    }
    for address in &addresses {
        driver
            .initialization()?
            .match_rom(address)?
            .write_scratchpad(&Scratchpad {
                alarm_high_trigger_register: 30,
                alarm_low_trigger_register: 10,
                configuration_register: ConfigurationRegister {
                    resolution: Resolution::Twelve,
                },
                ..Default::default()
            })?;
    }
    for address in &addresses {
        let scratchpad = driver
            .initialization()?
            .match_rom(address)?
            .read_scratchpad()?;
        info!("{address:x?}: {scratchpad:?}");
    }
    let (sender, mut receiver) = mpsc::channel::<Request>(9);
    info!("Spawn temperature reader");
    spawn(async move {
        while let Some((indices, sender)) = receiver.recv().await {
            trace!("Read temperatures {indices:?}");
            let _ = sender.send((|| {
                trace!("Send temperatures {indices:?}");
                if indices.end > addresses.len() {
                    return Err(Error::InvalidIndex {
                        received: indices,
                        expected: 0..addresses.len(),
                    });
                }
                let mut temperatures = BTreeMap::new();
                driver.initialization()?.skip_rom()?.convert_temperature()?;
                for index in indices {
                    let address = &addresses[index];
                    let temperature = driver
                        .initialization()?
                        .match_rom(address)?
                        .read_scratchpad()?
                        .temperature;
                    trace!("{address:x?}: {temperature}");
                    temperatures.insert(address.address(), temperature);
                }
                Ok(temperatures)
            })());
        }
    });
    Ok(sender)
}

/// Result
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Error
#[derive(Error, Debug)]
pub enum Error {
    #[error("Invalid index {{ received: {received:?}, expected: {expected:?} }}")]
    InvalidIndex {
        received: Range<usize>,
        expected: Range<usize>,
    },
    #[error(transparent)]
    Internal(#[from] thermometer::Error),
}

impl From<Error> for ExceptionCode {
    fn from(value: Error) -> Self {
        match value {
            Error::InvalidIndex { .. } => ExceptionCode::IllegalDataAddress,
            Error::Internal(_) => ExceptionCode::ServerDeviceFailure,
        }
    }
}
