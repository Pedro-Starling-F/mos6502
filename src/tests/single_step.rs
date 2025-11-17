use serde::{Deserialize, Serialize};
pub type Root = Vec<Root2>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Root2 {
    pub name: String,
    pub initial: Initial,
    #[serde(rename = "final")]
    pub final_field: Final,
    pub cycles: Vec<(i64, i64, String)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Initial {
    pub pc: i64,
    pub s: i64,
    pub a: i64,
    pub x: i64,
    pub y: i64,
    pub p: i64,
    pub ram: Vec<Vec<i64>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Final {
    pub pc: i64,
    pub s: i64,
    pub a: i64,
    pub x: i64,
    pub y: i64,
    pub p: i64,
    pub ram: Vec<Vec<i64>>,
}
