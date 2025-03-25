use esp_idf_svc::{
    eventloop::{EspEventLoop, System},
    hal::modem::Modem,
    nvs::EspDefaultNvsPartition,
    sys::EspError,
    timer::{EspTimerService, Task},
    wifi::{AsyncWifi, ClientConfiguration, Configuration, EspWifi},
};
use log::info;

const SSID: &str = env!("SSID");
const PASSWORD: &str = env!("PASSWORD");

pub(super) async fn connect(
    modem: Modem,
    event_loop: EspEventLoop<System>,
    timer: EspTimerService<Task>,
    nvs: Option<EspDefaultNvsPartition>,
) -> Result<EspWifi<'static>, EspError> {
    let mut esp_wifi = EspWifi::new(modem, event_loop.clone(), nvs.clone())?;
    let mut wifi = AsyncWifi::wrap(&mut esp_wifi, event_loop.clone(), timer)?;
    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: SSID.try_into().unwrap(),
        password: PASSWORD.try_into().unwrap(),
        ..Default::default()
    }))?;

    wifi.start().await?;
    info!("Wifi started");
    wifi.connect().await?;
    info!("Wifi connected");
    wifi.wait_netif_up().await?;
    info!("Wifi netif up");
    Ok(esp_wifi)
}
