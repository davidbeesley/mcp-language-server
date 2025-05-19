// This is a sample Rust file for testing

/// A simple struct for testing
pub struct TestStruct {
    /// The name field
    pub name: String,
    /// The value field
    pub value: i32,
}

impl TestStruct {
    /// Creates a new TestStruct
    pub fn new(name: &str, value: i32) -> Self {
        Self {
            name: name.to_string(),
            value,
        }
    }

    /// Gets the name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Gets the value
    pub fn value(&self) -> i32 {
        self.value
    }
}

/// A simple function for testing
pub fn test_function(input: i32) -> i32 {
    input * 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let test = TestStruct::new("test", 42);
        assert_eq!(test.name, "test");
        assert_eq!(test.value, 42);
    }

    #[test]
    fn test_getters() {
        let test = TestStruct::new("test", 42);
        assert_eq!(test.name(), "test");
        assert_eq!(test.value(), 42);
    }

    #[test]
    fn test_function_works() {
        assert_eq!(test_function(21), 42);
    }
}