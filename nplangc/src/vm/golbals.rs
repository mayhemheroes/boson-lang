use std::rc::Rc;

use crate::types;
use types::object::Object;

pub struct GolbalPool {
    pub pool: Vec<Rc<Object>>,
    pub current_size: usize,
    pub max_size: usize,
}