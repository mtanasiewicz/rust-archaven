use crate::app::orders::domain::order::Order;

pub struct OrderRepository;

impl OrderRepository {
    pub fn save(&self, _order: &Order) {}
}
