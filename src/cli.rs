use crate::cli::ParserResult::ParserError;
use menu::{Item, ItemType, Menu, Parameter};
use stm32f0xx_hal::{
    gpio::{gpiob, Alternate, AF0},
    serial,
};

type Output = SerialOutput;

const MENU: Menu<Output> = Menu {
    label: "root",
    items: &[&Item {
        command: "time",
        help: Some("Retrieve the current internal time"),
        item_type: ItemType::Callback {
            function: command_time,
            parameters: &[Parameter::Optional {
                parameter_name: "new_time",
                help: Some("if specified set the internal time to the given time"),
            }],
        },
    }],

    entry: None,
    exit: None,
};

struct SerialOutput {
    tx: serial::Tx<gpiob::PB6<Alternate<AF0>>>,
}

fn command_time(_menu: &Menu<Output>, item: &Item<Output>, args: &[&str], context: &mut Output) {
    if let Some(time) = ::menu::argument_finder(item, args, "new_time") {
        // set new time
    }

    // print current time
}

pub enum ParserResult {
    ParserError,
    NeedMoreData,
    ReadTime,
    SetTime,
}

struct CLI {}

impl CLI {
    pub fn next_char(&mut self, c: u8) -> ParserResult {
        /// add the next read character to the internal state
        ParserError
    }
}
