#!/bin/bash

# Exit script on any error
set -e

if ! command -v brew &> /dev/null; then
  echo "Error: Homebrew is not installed. Please install Homebrew first."
  echo "Alternatively you can look inside unix-setup.sh and install dependencies manually"
  exit 1
fi

# Determine the correct pip command
if command -v pip &> /dev/null; then
  PIP_CMD="pip"
elif command -v pip3 &> /dev/null; then
  PIP_CMD="pip3"
else
  echo "Error: Neither pip nor pip3 is installed. Please install pip to proceed."
  exit 1
fi

# Install SQLite
echo "Installing SQLite..."
brew install sqlite

# Install protobuf compiler
echo "Installing protobuf compiler..."
brew install protobuf

# Install pre-commit
echo "Installing pre-commit..."
$PIP_CMD install pre-commit

# Initialize pre-commit hooks
echo "Initializing pre-commit hooks..."
pre-commit install

echo "All dependencies installed successfully!"
