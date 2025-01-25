use bcrypt::{hash, verify, DEFAULT_COST};

#[derive(Debug, Clone)]
pub struct Password {
    hashed_password: String,
}

impl Password {
    /// Creates a new `Password` instance by hashing the provided plaintext password.
    ///
    /// # Arguments
    /// * `plaintext` - The plaintext password to hash.
    ///
    /// # Returns
    /// A `Result` containing the `Password` instance or an error if hashing fails.
    pub fn new(plaintext: &str) -> Result<Self, bcrypt::BcryptError> {
        let hashed = hash(plaintext, DEFAULT_COST)?;
        Ok(Self {
            hashed_password: hashed,
        })
    }

    pub fn from_hash(hash: &str) -> Self {
        Self {
            hashed_password: hash.to_string(),
        }
    }

    /// Verifies a plaintext password against the hashed password.
    ///
    /// # Arguments
    /// * `plaintext` - The plaintext password to verify.
    ///
    /// # Returns
    /// A `Result` containing `true` if the password matches, `false` otherwise,
    /// or an error if verification fails.
    pub fn verify(&self, plaintext: &str) -> Result<bool, bcrypt::BcryptError> {
        verify(plaintext, &self.hashed_password)
    }

    /// Gets the hashed password as a reference string.
    pub fn hashed(&self) -> &str {
        &self.hashed_password
    }
}

#[cfg(test)]
mod tests {
    use super::Password;

    #[test]
    fn test_password_hashing_and_verification() {
        let plaintext = "my_secure_password";

        // Create a new Password instance
        let password = Password::new(plaintext).unwrap();

        // Verify that the hashed password matches the plaintext
        assert!(password.verify(plaintext).unwrap());

        // Verify that an incorrect password does not match
        assert!(!password.verify("wrong_password").unwrap());
    }
}
