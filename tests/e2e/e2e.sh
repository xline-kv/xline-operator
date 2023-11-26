#!/bin/bash

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/common/common.sh"
source "$(dirname "${BASH_SOURCE[0]}")/testenv/testenv.sh"
source "$(dirname "${BASH_SOURCE[0]}")/cases/cases.sh"

function setup() {
  testenv::k8s::create
  testenv::util::install
}

function teardown() {
  testenv::k8s::delete
}

function list_test_cases() {
  local -a functions
  IFS=$'\n' read -d '' -ra functions <<<"$(compgen -A function | sort)" && unset IFS
  local -a testcases=()
  for func in "${functions[@]}"; do
    if [[ "$func" =~ ^test::run:: ]]; then
      testcase=${func#test::run::}
      if [[ -n "${E2E_TEST_CASE_PREFIX:=}" && ${testcase} != "${E2E_TEST_CASE_PREFIX}"* ]]; then
        continue
      fi
      testcases+=("$testcase")
    fi
  done
  echo -n "${testcases[*]}"
}

function run() {
  local -a testcases=()
  IFS=" " read -ra testcases <<<"$(list_test_cases)" && unset IFS
  local failed=0
  local passed=0
  for testcase in "${testcases[@]}"; do
    log::info "=== Running test case: $testcase ==="
    if test::run::"$testcase"; then
      log::info "Test case passed: $testcase"
      ((passed++))
    else
      log::error "Test case failed: $testcase"
      ((failed++))
    fi
  done
  if ((failed > 0)); then
    log::error "Failed test cases: $failed/${#testcases[@]}"
    return "${failed}"
  else
    log::info "All test cases passed"
  fi
}

function help() {
  echo "Xline Operator E2E Test Script"
  echo ""
  echo "Parameters:"
  echo "  -p <prefix>  Run selected test cases with prefix"
  echo "  -h           Print this help"
  echo "  -l           List all test cases"
  echo "  -c           Clean the kind cluster."
}

function main() {
  while getopts "p:lhc" opt; do
    case "$opt" in
    p)
      export E2E_TEST_CASE_PREFIX="$OPTARG"
      log::info "Run selected test cases with prefix: $E2E_TEST_CASE_PREFIX"
      ;;
    l)
      for testcase in $(list_test_cases); do
        echo "$testcase"
      done
      exit 0
      ;;
    c)
      teardown
      exit 0
      ;;
    h)
      help
      exit 0
      ;;
    ?) ;;
    esac
  done

  setup || return $?
  run || return $?
  teardown
}

main "$@"
