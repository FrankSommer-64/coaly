// -----------------------------------------------------------------------------------------------
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
// -----------------------------------------------------------------------------------------------

//! Special collection types for Coaly.

use std::collections::BTreeMap;
use std::collections::btree_map::Values;
use std::fmt::{Debug, Formatter};

/// Stack with maximum capacity and a defined overflow behaviour.
/// After maximum capacity is exceeded, further push operations will just increase a counter
/// but otherwise be ignored.
/// If overflow state terminates after sufficient pop operations the stack returns to ordinary
/// behaviour.
/// Used for Coaly's mode change stack, to cope with recursive function calls. 
#[derive(Clone)]
pub(crate) struct RecoverableStack<T> {
    // vector holding the stack elements during non-overflow operation
    items: Vec<T>,
    // number of push operations after the stack reached overflow state
    overflow_count: usize,
    // stack capacity, stack enters overflow state if exceeded
    max_capacity: usize
}

impl<T> RecoverableStack<T> {
    /// Creates a recoverable stack with specified maximum and initial capacity.
    ///
    /// # Arguments
    /// * `max_capacity` - the maximum capacity of the stack, before entering overflow state
    /// * `initial_capacity` - the initial capacity of the stack
    #[inline]
    pub(crate) fn new(max_capacity: usize,
                      initial_capacity: usize) -> RecoverableStack<T> {
        RecoverableStack {
            items: Vec::with_capacity(initial_capacity),
            overflow_count: 0,
            max_capacity
        }
    }

    /// Pushes an element to the top of the stack.
    ///
    /// # Arguments
    /// * `value` - the value to push
    ///
    /// # Return values
    /// **true** if the value was appended, **false** if an overflow occurred
    pub(crate) fn push(&mut self, value: T) -> bool {
        if self.items.len() >= self.max_capacity {
            // usize overflow will panic, but since push is called whenever a function is called,
            // a stack overflow will happen long before
            self.overflow_count += 1;
            return false
        }
        self.items.push(value);
        true
    }

    /// Removes the top element from a stack and returns it.
    ///
    /// # Return values
    /// **top element** of the stack, **None** if the stack is in overflow state or empty
    pub(crate) fn pop(&mut self) -> Option<T> {
        if self.overflow_count == 0 { return self.items.pop() }
        self.overflow_count -= 1;
        None
    }

    /// Returns the top element from a stack and returns it.
    ///
    /// # Return values
    /// **top element** of the stack, **None** if the stack is empty
    #[inline]
    pub(crate) fn last(&self) -> Option<&T> { self.items.last() }
}
impl<T> Debug for RecoverableStack<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "CAP:{}/LEN:{}/OFL:{}", self.max_capacity, self.items.len(), self.overflow_count)
    }
}

/// Generic type containing a map with custom elements, and a separate element
/// acting as default, if no custom elements exist.
#[derive(Clone)]
pub(crate) struct MapWithDefault<T> {
    default_element: T,
    custom_elements: BTreeMap<String, T>
} 
impl<T> MapWithDefault<T> {
    /// Returns the element with the given name.
    /// If the map doesn't contain one with that name, returns the default element.
    ///
    /// # Arguments
    /// * `name` - the element name
    #[inline]
    pub(crate) fn get(&self, name: &str) -> &T {
        self.custom_elements.get(name).unwrap_or(&self.default_element)
    }

    /// Returns the element with the given name.
    /// If the name is None or the map doesn't contain one with that name,
    /// returns the default element.
    ///
    /// # Arguments
    /// * `name` - the element name
    #[inline]
    pub(crate) fn find(&self, name: &Option<String>) -> &T {
        if let Some(n) = name { return self.get(n) }
        &self.default_element
    }

    /// Inserts a custom element into the map.
    /// 
    /// # Arguments
    /// * `name` - the name of the element
    /// * `desc` - the element
    ///
    /// # Return values
    /// the element in the map, that was replaced by the new one; **None**, if the map didn't
    /// contain an element with the specified key
    #[inline]
    pub(crate) fn insert(&mut self, name: &str, element: T) -> Option<T> {
        self.custom_elements.insert(name.to_string(), element)
    }

    /// Returns an iterator over the custom values of the map.
    #[inline]
    pub(crate) fn custom_values(&self) -> Values<String, T> {
        self.custom_elements.values()
    }
}
impl<T: Default> Default for MapWithDefault<T> {
    fn default() -> Self {
        MapWithDefault {
            default_element: T::default(),
            custom_elements: BTreeMap::<String, T>::new()
        }
    }
}
impl<T: Debug> Debug for MapWithDefault<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut buf = String::with_capacity(512);
        for (fmt_name, fmt) in self.custom_elements.iter() {
            if ! buf.is_empty() { buf.push(','); }
            buf.push_str(&format!("{{{}:{:?}}}", fmt_name, fmt));
        }
        write!(f, "DEF:{{{:?}}}/CUST:{}", self.default_element, buf)
    }
}

/// Generic type containing a vector with custom elements, and a separate element
/// acting as default, if no custom elements exist.
#[derive(Clone)]
pub(crate) struct VecWithDefault<T> {
    default_element: T,
    custom_elements: Vec<T>
}
impl<T> VecWithDefault<T> {
    /// Returns an iterator over the custom elements.
    #[inline]
    pub(crate) fn custom_elements(&self) -> std::slice::Iter<'_, T> { self.custom_elements.iter() }

    /// Returns an iterator over the custom elements, if any, otherwise default element.
    pub(crate) fn elements(&self) -> std::slice::Iter<'_, T> {
        if self.custom_elements.is_empty() {
            std::slice::from_ref(&self.default_element).iter()
        } else { self.custom_elements.iter() }
    }

    /// Adds a custom element to the end of the vector.
    /// 
    /// # Arguments
    /// * `element` - the element to add
    #[inline]
    pub(crate) fn push(&mut self, element: T) { self.custom_elements.push(element) }

    /// Returns the default element.
    #[cfg(test)]
    #[inline]
    fn default_element(&self) -> &T { &self.default_element }

    /// Indicates whether the vector contains custom elements
    #[cfg(test)]
    #[inline]
    fn has_custom_elements(&self) -> bool { ! self.custom_elements.is_empty() }
}
impl<T: Default> Default for VecWithDefault<T> {
    fn default() -> Self {
        VecWithDefault {
            default_element: T::default(),
            custom_elements: Vec::<T>::new()
        }
    }
}
impl<T: Debug> Debug for VecWithDefault<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut buf = String::with_capacity(512);
        for elem in self.custom_elements.iter() {
            if ! buf.is_empty() { buf.push(','); }
            buf.push_str(&format!("{{{:?}}}", elem));
        }
        write!(f, "DEF:{{{:?}}}/CUST:{}", self.default_element, buf)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[derive(Clone,PartialEq)]
    struct TestStruct {
        name: String,
        value: u32
    }
    impl Default for TestStruct {
        fn default() -> Self {
            TestStruct {
                name: String::from("default"),
                value: 0
            }
        }
    }
    impl Debug for TestStruct {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            write!(f, "VAL:{}", self.value)
        }
    }

    #[test]
    fn test_recoverable_stack() {
        let mut stack = RecoverableStack::<u32>::new(4, 4);
        // empty stack
        assert_eq!("CAP:4/LEN:0/OFL:0", &format!("{:?}", &stack));
        assert!(stack.last().is_none());
        assert!(stack.pop().is_none());
        assert_eq!("CAP:4/LEN:0/OFL:0", &format!("{:?}", &stack));

        // one element
        let mut stack = RecoverableStack::<u32>::new(4, 4);
        stack.push(123);
        assert_eq!("CAP:4/LEN:1/OFL:0", &format!("{:?}", &stack));
        assert!(stack.last().is_some());
        assert!(stack.pop().is_some());
        assert_eq!("CAP:4/LEN:0/OFL:0", &format!("{:?}", &stack));

        // one element below max capacity
        let mut stack = RecoverableStack::<u32>::new(4, 4);
        stack.push(123);
        stack.push(123);
        stack.push(123);
        assert_eq!("CAP:4/LEN:3/OFL:0", &format!("{:?}", &stack));
        stack.push(123);
        assert_eq!("CAP:4/LEN:4/OFL:0", &format!("{:?}", &stack));
        assert!(stack.last().is_some());
        assert!(stack.pop().is_some());
        assert_eq!("CAP:4/LEN:3/OFL:0", &format!("{:?}", &stack));

        // at max capacity
        let mut stack = RecoverableStack::<u32>::new(4, 4);
        stack.push(123);
        stack.push(123);
        stack.push(123);
        stack.push(123);
        assert_eq!("CAP:4/LEN:4/OFL:0", &format!("{:?}", &stack));
        stack.push(123);
        assert_eq!("CAP:4/LEN:4/OFL:1", &format!("{:?}", &stack));
        assert!(stack.last().is_some());
        assert!(stack.pop().is_none());
        assert_eq!("CAP:4/LEN:4/OFL:0", &format!("{:?}", &stack));

        // above max capacity
        let mut stack = RecoverableStack::<u32>::new(4, 4);
        stack.push(123);
        stack.push(123);
        stack.push(123);
        stack.push(123);
        stack.push(123);
        stack.push(123);
        assert_eq!("CAP:4/LEN:4/OFL:2", &format!("{:?}", &stack));
        stack.push(123);
        assert_eq!("CAP:4/LEN:4/OFL:3", &format!("{:?}", &stack));
        assert!(stack.last().is_some());
        assert!(stack.pop().is_none());
        assert_eq!("CAP:4/LEN:4/OFL:2", &format!("{:?}", &stack));
    }

    #[test]
    fn test_map_with_default() {
        // empty map
        let map = MapWithDefault::<TestStruct>::default();
        assert_eq!("DEF:{VAL:0}/CUST:", &format!("{:?}", &map));
        assert!(map.custom_values().clone().next().is_none());
        assert_eq!(TestStruct::default(), *map.get("xyz"));
        assert_eq!(TestStruct::default(), *map.find(&Some(String::from("xyz"))));
        assert_eq!(TestStruct::default(), *map.find(&None));

        // map with custom element
        let mut map = MapWithDefault::<TestStruct>::default();
        let cust_elem = TestStruct { name: String::from("custom"), value: 123 };
        map.insert("custom", cust_elem.clone());
        assert_eq!("DEF:{VAL:0}/CUST:{custom:VAL:123}", &format!("{:?}", &map));
        assert!(map.custom_values().clone().next().is_some());
        assert_eq!(TestStruct::default(), *map.get("xyz"));
        assert_eq!(cust_elem, *map.find(&Some(String::from("custom"))));
        assert_eq!(TestStruct::default(), *map.find(&Some(String::from("xyz"))));
        assert_eq!(TestStruct::default(), *map.find(&None));
    }

    #[test]
    fn test_vec_with_default() {
        // empty vector
        let v = VecWithDefault::<TestStruct>::default();
        assert_eq!(TestStruct::default(), *v.default_element());
        assert!(! v.has_custom_elements());
        assert!(v.custom_elements().clone().next().is_none());
        assert_eq!("DEF:{VAL:0}/CUST:", &format!("{:?}", &v));

        // map with custom element
        let mut v = VecWithDefault::<TestStruct>::default();
        let cust_elem = TestStruct { name: String::from("custom"), value: 123 };
        v.push(cust_elem.clone());
        assert_eq!("DEF:{VAL:0}/CUST:{VAL:123}", &format!("{:?}", &v));
        assert!(v.has_custom_elements());
        assert!(v.custom_elements().clone().next().is_some());
        assert_eq!(cust_elem, *v.custom_elements().clone().next().unwrap());
    }
}