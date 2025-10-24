use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub wallpapers: Wallpapers, //
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Wallpapers {
    pub days: Days, //
    pub dates: HashMap<String, String>, //
    pub periods: Vec<Period>, //
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Days { //
    pub monday: String,
    pub tuesday: String,
    pub wednesday: String,
    pub thursday: String,
    pub friday: String,
    pub saturday: String,
    pub sunday: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Period { //
    pub day: String,
    pub start: String,
    pub end: String,
    pub url: String,
}