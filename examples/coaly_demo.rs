// ---------------------------------------------------------------------------------------------
// Coaly - context aware logging and tracing system
//
// Copyright (c) 2022, Frank Sommer.
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// * Redistributions of source code must retain the above copyright notice, this
//   list of conditions and the following disclaimer.
//
// * Redistributions in binary form must reproduce the above copyright notice,
//   this list of conditions and the following disclaimer in the documentation
//   and/or other materials provided with the distribution.
//
// * Neither the name of the copyright holder nor the names of its
//   contributors may be used to endorse or promote products derived from
//   this software without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
// FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
// DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
// SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
// CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
// OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
// ---------------------------------------------------------------------------------------------

//! Simple demo application showing API usage.

use std::fmt::{Display, Formatter};
use coaly::*;

pub fn main() {
    // initialize from custom configuration file coaly_demo.toml
    coaly::initialize("examples/coaly_demo.toml");
    loginfo!("Coaly demo started");
    for i in 121..=124 {
        let my_order = Order::new(&i.to_string());
        mymod::process(&my_order);
    }
    loginfo!("Coaly demo terminated");
    coaly::shutdown();
}

/// Order structure processed by demo application.
/// Implements CoalyObservable to allow mode changes based on creation and drop of
/// a custom application structure.
pub struct Order {
    _id: String,
    obs: CoalyObserver
}
impl Order {
    fn new (id: &str) -> Order { Order { _id: id.to_string(), obs: newcoalyobs!(id, id) } }
}
impl CoalyObservable for Order {
    fn coaly_observer(&self) -> &CoalyObserver { &self.obs }
}
impl Display for Order {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result { write!(f, "Order({})", self._id) }
}

/// A demo module
mod mymod {
    use super::Order;
    use coaly::*;

    /// Processes an order structure.
    pub fn process(order: &Order) {
        // module and function boundaries are logged for order '123' only, since the custom
        // configuration file contains a mode change enabling all record levels for that order
        logmod!("mymod");
        logfn!("mymod::process", order);
        calc(order);
        // this info message is logged for every order
        loginfo!("{} processed", order);
    }

    /// Calculate something 
    fn calc(order: &Order) {
        // again, function entry/exit and debug message is logged for order '123' only
        logfn!("mymod::calc", order);
        logdebug!("calc");
    }
}
