#! /bin/bash
# This sets the necessary environment variables for packaging the application for distribution

echo "Setting environment variables"

# See README
export APPLE_SIGNING_IDENTITY="<the result of `security find-identity -v -p codesigning`>"
export APPLE_ID="<swag@icloud.gov>"
export APPLE_PASSWORD="<app-specific password>"
export APPLE_TEAM_ID="<team ID, found in Apple Developer dashboard>"

echo "Environment variables set"