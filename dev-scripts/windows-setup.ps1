# Exit script on error
$ErrorActionPreference = "Stop"

# Install SQLite
Write-Host "Installing SQLite..."
choco install sqlite -y

# Install protobuf compiler
Write-Host "Installing protobuf compiler..."
choco install protoc -y

# Install pre-commit
Write-Host "Installing pre-commit..."
pip install pre-commit

# Initialize pre-commit hooks
Write-Host "Initializing pre-commit hooks..."
pre-commit install

Write-Host "All dependencies installed successfully!"

