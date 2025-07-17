#!/bin/bash
set -euo pipefail

echo "Running CLI integration tests..."

# Use dotenv provider for testing
export SECRETSPEC_PROVIDER=dotenv

# Test directory for isolated tests
TEST_DIR="$(mktemp -d)"
cd "$TEST_DIR"

# Helper function to check command success
check_success() {
    if [ $? -eq 0 ]; then
        echo "✓ $1"
    else
        echo "✗ $1"
        exit 1
    fi
}

# Helper function to check command failure
check_failure() {
    if [ $? -ne 0 ]; then
        echo "✓ $1"
    else
        echo "✗ $1"
        exit 1
    fi
}

# Test 1: Help command
secretspec --help > /dev/null
check_success "Help command works"

# Test 2: Version command
secretspec --version > /dev/null
check_success "Version command works"

# Test 3: Init command
secretspec init
check_success "Init command creates secretspec.toml"

# Verify the file was created
[ -f "secretspec.toml" ]
check_success "secretspec.toml file exists"

# Test 4: Declare and set a secret
cat > secretspec.toml << EOF
[project]
name = "test-app"
revision = "1.0"

[profiles.default]
TEST_SECRET = { description = "Test secret for integration tests" }
EOF

echo "test_value" | secretspec set TEST_SECRET
check_success "Set TEST_SECRET"

# Get the secret
VALUE=$(secretspec get TEST_SECRET)
[ "$VALUE" = "test_value" ]
check_success "Get TEST_SECRET returns correct value"

# Test 5: Check command with missing required secret
cat > secretspec.toml << EOF
[project]
name = "test-app"
revision = "1.0"

[profiles.default]
TEST_SECRET = { description = "Test secret for integration tests" }
REQUIRED_SECRET = { description = "Required secret", required = true }
EOF

secretspec check 2>/dev/null || true
check_failure "Check fails with missing required secret"

# Set the required secret
echo "required_value" | secretspec set REQUIRED_SECRET
check_success "Set REQUIRED_SECRET"

# Now check should pass
secretspec check
check_success "Check passes with all required secrets"

# Test 6: Import from .env file
cat > .env.import << EOF
ENV_VAR1=value1
ENV_VAR2=value2
EOF

# First declare the secrets we're importing
cat > secretspec.toml << EOF
[project]
name = "test-app"
revision = "1.0"

[profiles.default]
TEST_SECRET = { description = "Test secret" }
REQUIRED_SECRET = { description = "Required secret", required = true }
ENV_VAR1 = { description = "Imported from .env" }
ENV_VAR2 = { description = "Imported from .env" }
EOF

secretspec import --from .env.import
check_success "Import from .env file"

# Verify imported values
VALUE1=$(secretspec get ENV_VAR1)
VALUE2=$(secretspec get ENV_VAR2)
[ "$VALUE1" = "value1" ] && [ "$VALUE2" = "value2" ]
check_success "Imported values are correct"

# Test 7: Run command with secrets
echo "#!/bin/bash" > test_script.sh
echo "echo \"\$TEST_SECRET\"" >> test_script.sh
chmod +x test_script.sh

OUTPUT=$(secretspec run -- ./test_script.sh)
[ "$OUTPUT" = "test_value" ]
check_success "Run command with secrets injected"

# Test 8: Profile support
secretspec --profile production init
check_success "Init with production profile"

# Declare secret in production profile
cat >> secretspec.toml << EOF

[profiles.production]
PROD_SECRET = { description = "Production secret" }
EOF

echo "prod_value" | secretspec --profile production set PROD_SECRET
check_success "Set secret in production profile"

# Test 9: List secrets
secretspec list > /dev/null
check_success "List secrets command works"

# Test 10: Config command
secretspec config > /dev/null
check_success "Config command works"

# Test 11: Default value handling
cat > secretspec.toml << EOF
[project]
name = "test-app"
revision = "1.0"

[profiles.default]
DEFAULT_SECRET = { description = "Secret with default", default = "default_value" }
EOF

# Should use default value when not set
VALUE=$(secretspec get DEFAULT_SECRET)
[ "$VALUE" = "default_value" ]
check_success "Default value is used when secret not set"

# Cleanup
cd ..
rm -rf "$TEST_DIR"

echo "All CLI integration tests passed!"