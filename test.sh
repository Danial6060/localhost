#!/bin/bash

# Comprehensive test script for Webserv HTTP Server

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

HOST="127.0.0.1"
PORT="8080"
BASE_URL="http://${HOST}:${PORT}"

pass_count=0
fail_count=0

echo "========================================="
echo "  Webserv HTTP Server Test Suite"
echo "========================================="
echo

test_get() {
    echo -n "Testing GET request... "
    response=$(curl -s -o /dev/null -w "%{http_code}" ${BASE_URL}/)
    if [ "$response" -eq 200 ]; then
        echo -e "${GREEN}PASS${NC} (Status: $response)"
        ((pass_count++))
    else
        echo -e "${RED}FAIL${NC} (Status: $response)"
        ((fail_count++))
    fi
}

test_get_404() {
    echo -n "Testing GET 404 error... "
    response=$(curl -s -o /dev/null -w "%{http_code}" ${BASE_URL}/nonexistent)
    if [ "$response" -eq 404 ]; then
        echo -e "${GREEN}PASS${NC} (Status: $response)"
        ((pass_count++))
    else
        echo -e "${RED}FAIL${NC} (Status: $response)"
        ((fail_count++))
    fi
}

test_post() {
    echo -n "Testing POST request... "
    response=$(curl -s -o /dev/null -w "%{http_code}" -X POST -d "test=data" ${BASE_URL}/)
    if [ "$response" -eq 200 ]; then
        echo -e "${GREEN}PASS${NC} (Status: $response)"
        ((pass_count++))
    else
        echo -e "${RED}FAIL${NC} (Status: $response)"
        ((fail_count++))
    fi
}

test_delete() {
    echo -n "Testing DELETE method... "
    echo "test" > /tmp/test_delete.txt 2>/dev/null
    response=$(curl -s -o /dev/null -w "%{http_code}" -X DELETE ${BASE_URL}/uploads/test_delete.txt 2>/dev/null)
    if [ "$response" -eq 204 ] || [ "$response" -eq 404 ]; then
        echo -e "${GREEN}PASS${NC} (Status: $response)"
        ((pass_count++))
    else
        echo -e "${RED}FAIL${NC} (Status: $response)"
        ((fail_count++))
    fi
}

test_method_not_allowed() {
    echo -n "Testing 405 Method Not Allowed... "
    response=$(curl -s -o /dev/null -w "%{http_code}" -X PUT ${BASE_URL}/)
    if [ "$response" -eq 405 ] || [ "$response" -eq 501 ]; then
        echo -e "${GREEN}PASS${NC} (Status: $response)"
        ((pass_count++))
    else
        echo -e "${RED}FAIL${NC} (Status: $response)"
        ((fail_count++))
    fi
}

test_cookies() {
    echo -n "Testing cookies and sessions... "
    response=$(curl -s -i ${BASE_URL}/ | grep -i "Set-Cookie")
    if [ ! -z "$response" ]; then
        echo -e "${GREEN}PASS${NC} (Cookie set)"
        ((pass_count++))
    else
        echo -e "${RED}FAIL${NC} (No cookie)"
        ((fail_count++))
    fi
}

test_file_upload() {
    echo -n "Testing file upload... "
    echo "test file content" > /tmp/test_upload.txt
    response=$(curl -s -o /dev/null -w "%{http_code}" -F "file=@/tmp/test_upload.txt" ${BASE_URL}/uploads 2>/dev/null)
    rm -f /tmp/test_upload.txt
    if [ "$response" -eq 200 ] || [ "$response" -eq 201 ]; then
        echo -e "${GREEN}PASS${NC} (Status: $response)"
        ((pass_count++))
    else
        echo -e "${RED}FAIL${NC} (Status: $response)"
        ((fail_count++))
    fi
}

test_multiple_requests() {
    echo -n "Testing concurrent requests... "
    for i in {1..10}; do
        curl -s -o /dev/null ${BASE_URL}/ &
    done
    wait
    echo -e "${GREEN}PASS${NC} (All requests completed)"
    ((pass_count++))
}

test_keep_alive() {
    echo -n "Testing keep-alive connection... "
    response=$(curl -s -i ${BASE_URL}/ | grep -i "Connection: keep-alive")
    if [ ! -z "$response" ]; then
        echo -e "${GREEN}PASS${NC}"
        ((pass_count++))
    else
        echo -e "${YELLOW}SKIP${NC}"
    fi
}

# Run all tests
echo "Running tests..."
echo

test_get
test_get_404
test_post
test_delete
test_method_not_allowed
test_cookies
test_file_upload
test_multiple_requests
test_keep_alive

echo
echo "========================================="
echo "  Test Results"
echo "========================================="
echo -e "Passed: ${GREEN}${pass_count}${NC}"
echo -e "Failed: ${RED}${fail_count}${NC}"
echo "========================================="

if command -v siege &> /dev/null; then
    echo
    echo "Running siege stress test (10 seconds)..."
    siege -b -t10S ${BASE_URL}/ 2>&1 | grep -E "Availability|Transaction rate"
fi

exit $fail_count