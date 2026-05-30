use crate::app::orders::application::create_order::CreateOrder;

pub fn create_order() {
    let command = CreateOrder;
    let _order = command.handle();
}
