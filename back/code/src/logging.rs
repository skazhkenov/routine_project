use log4rs::config::Config;
use crate::LOGS_CONFIG_FILE;

pub fn init_logger() {

    let log_config: Config = log4rs::config::load_config_file(
        LOGS_CONFIG_FILE, 
        Default::default()
    ).unwrap();
    let _: log4rs::Handle = log4rs::init_config(log_config).unwrap();
}