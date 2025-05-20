use anyhow::Result;
use assert_fs::TempDir;
use assert_fs::fixture::PathChild;
use std::path::{Path, PathBuf};
use tokio::fs;

/// Creates a test file with the specified content
pub async fn create_test_file(dir: &TempDir, name: &str, content: &str) -> Result<PathBuf> {
    let file_path = dir.child(name).path().to_path_buf();
    fs::write(&file_path, content).await?;
    Ok(file_path)
}

/// Reads a file and returns its content
pub async fn read_file_content(path: &Path) -> Result<String> {
    let content = fs::read_to_string(path).await?;
    Ok(content)
}

/// A simple Rust file for testing
pub fn sample_rust_file() -> &'static str {
    r#"
// A sample Rust file for testing
struct User {
    name: String,
    email: String,
}

impl User {
    fn new(name: &str, email: &str) -> Self {
        Self {
            name: name.to_string(),
            email: email.to_string(),
        }
    }
    
    fn greet(&self) -> String {
        format!("Hello, {}!", self.name)
    }
}

fn main() {
    let user = User::new("Alice", "alice@example.com");
    println!("{}", user.greet());
}
"#
}

/// A Rust file with a more complex structure
pub fn complex_rust_file() -> &'static str {
    r#"
// A more complex Rust file for testing
use std::collections::HashMap;
use std::fmt::{self, Display};

#[derive(Debug, Clone)]
pub struct Person {
    name: String,
    age: u32,
    attributes: HashMap<String, String>,
}

#[derive(Debug)]
pub enum Role {
    User,
    Admin,
    Guest,
}

impl Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Role::User => write!(f, "User"),
            Role::Admin => write!(f, "Admin"),
            Role::Guest => write!(f, "Guest"),
        }
    }
}

impl Person {
    pub fn new(name: &str, age: u32) -> Self {
        Self {
            name: name.to_string(),
            age,
            attributes: HashMap::new(),
        }
    }
    
    pub fn add_attribute(&mut self, key: &str, value: &str) {
        self.attributes.insert(key.to_string(), value.to_string());
    }
    
    pub fn get_attribute(&self, key: &str) -> Option<&String> {
        self.attributes.get(key)
    }
    
    pub fn with_role(mut self, role: Role) -> Self {
        self.add_attribute("role", &role.to_string());
        self
    }
}

fn process_people(people: &[Person]) -> Vec<String> {
    people
        .iter()
        .map(|p| format!("{} ({})", p.name, p.age))
        .collect()
}

fn main() {
    let mut alice = Person::new("Alice", 30);
    alice.add_attribute("department", "Engineering");
    
    let bob = Person::new("Bob", 25).with_role(Role::Admin);
    
    let people = vec![alice, bob];
    let result = process_people(&people);
    
    for person in result {
        println!("{}", person);
    }
}
"#
}