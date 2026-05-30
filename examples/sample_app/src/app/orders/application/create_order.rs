use crate::app::orders::domain::order::Order;

pub struct CreateOrder;

impl CreateOrder {
    pub fn handle(&self) -> Order {
        Order
    }
}
