#![allow(unused_macros)]

use crate::{
    flow::{Calculator, Checker, Controller, ControllerGenKey, Rule, StandaloneStat},
    Result,
};
use std::sync::{Arc, Mutex, Weak};

// todo: use `syn` to concat the ident/expr name, further reduce the number of arguments
macro_rules! insert_flow_generator {
    ($map:expr, $calculator_strategy:expr, $controller_strategy:expr, $calculator_struct:ident, $checker_struct:ident) => {
        $map.insert(
            ControllerGenKey::new($calculator_strategy, $controller_strategy),
            Box::new(
                |rule: Arc<Rule>, stat: Option<Arc<StandaloneStat>>| -> Result<Arc<Controller>> {
                    let stat = match stat {
                        None => generate_stat_for(&rule)?,
                        Some(stat) => stat,
                    };
                    let calculator: Arc<Mutex<dyn Calculator>> = Arc::new(Mutex::new(
                        $calculator_struct::new(Weak::new(), Arc::clone(&rule)),
                    ));
                    let checker: Arc<Mutex<dyn Checker>> = Arc::new(Mutex::new(
                        $checker_struct::new(Weak::new(), Arc::clone(&rule)),
                    ));
                    let mut tsc = Controller::new(Arc::clone(&rule), stat);
                    tsc.set_calculator(Arc::clone(&calculator));
                    tsc.set_checker(Arc::clone(&checker));
                    let tsc = Arc::new(tsc);
                    let mut calculator = calculator.lock().unwrap();
                    let mut checker = checker.lock().unwrap();
                    calculator.set_owner(Arc::downgrade(&tsc));
                    checker.set_owner(Arc::downgrade(&tsc));
                    Ok(tsc)
                },
            ),
        );
    };
}
