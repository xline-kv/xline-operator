${__E2E_CASES_CI__:=false} && return 0 || __E2E_CASES_CI__=true

source "$(dirname "${BASH_SOURCE[0]}")/../common/common.sh"
source "$(dirname "${BASH_SOURCE[0]}")/../testenv/testenv.sh"

_TEST_CI_CLUSTER_NAME="my-xline-cluster"
_TEST_CI_STS_NAME="$_TEST_CI_CLUSTER_NAME-sts"
_TEST_CI_SVC_NAME="$_TEST_CI_CLUSTER_NAME-svc"
_TEST_CI_SECRET_NAME="auth-cred"
_TEST_CI_NAMESPACE="default"
_TEST_CI_DNS_SUFFIX="svc.cluster.local"
_TEST_CI_XLINE_PORT="2379"
_TEST_CI_STORAGECLASS_NAME="e2e-storage"
_TEST_CI_LOG_SYNC_TIMEOUT=30

function test::ci::_mk_endpoints() {
  local endpoints="${_TEST_CI_STS_NAME}-0.${_TEST_CI_SVC_NAME}.${_TEST_CI_NAMESPACE}.${_TEST_CI_DNS_SUFFIX}:${_TEST_CI_XLINE_PORT}"
  for ((i = 1; i < $1; i++)); do
    endpoints="${endpoints},${_TEST_CI_STS_NAME}-${i}.${_TEST_CI_SVC_NAME}.${_TEST_CI_NAMESPACE}.${_TEST_CI_DNS_SUFFIX}:${_TEST_CI_XLINE_PORT}"
  done
  echo "$endpoints"
}

function test::ci::_etcdctl_expect() {
  log::info "run command: etcdctl --endpoints=$1 $2"
  got=$(testenv::util::etcdctl --endpoints="$1" "$2")
  expect=$(echo -e "$3")
  if [ "${got//$'\r'/}" == "$expect" ]; then
    log::info "command $2 run success"
  else
    log::error "command $2 run failed"
    log::error "expect: $expect"
    log::error "got: $got"
    return 1
  fi
}

# run a command with expect output, based on key word match
# args:
#   $1: endpoints
#   $2: command to run
#   $3: key word to match
function test::ci::_etcdctl_match() {
    log::debug "run command: etcdctl --endpoints=$1 $2"
    got=$(testenv::util::etcdctl --endpoints="$1" "$2")
    expect=$(echo -e ${3})
    if echo "${got}" | grep -q "${expect}"; then
      log::info  "command run success"
    else
      log::error  "command run failed"
      log::error  "expect: ${expect}"
      log::error "got: $got"
      return 1
    fi
}

function test::ci::_auth_validation() {
  log::info "auth validation test running..."
  endpoints=$(test::ci::_mk_endpoints 3)
  test::ci::_etcdctl_expect "$endpoints" "user add root:root" "User root created" || return $?
  test::ci::_etcdctl_expect "$endpoints" "role add root" "Role root created" || return $?
  test::ci::_etcdctl_expect "$endpoints" "user grant-role root root" "Role root is granted to user root" || return $?
  test::ci::_etcdctl_match "$endpoints" "--user root:root user list" "etcdserver: authentication is not enabled" || return $?
  test::ci::_etcdctl_expect "$endpoints" "auth enable" "Authentication Enabled" || return $?
  test::ci::_etcdctl_match "$endpoints" "--user root:rot user list" "etcdserver: authentication failed, invalid user ID or password" || return $?
  test::ci::_etcdctl_expect "$endpoints" "--user root:root auth status" "Authentication Status: true\nAuthRevision: 4" || return $?
  test::ci::_etcdctl_expect "$endpoints" "--user root:root user add u:u" "User u created" || return $?
  test::ci::_etcdctl_match "$endpoints" "--user u:u user add f:f" "etcdserver: permission denied" || return $?
  test::ci::_etcdctl_expect "$endpoints" "--user root:root role add r" "Role r created" || return $?
  test::ci::_etcdctl_expect "$endpoints" "--user root:root user grant-role u r" "Role r is granted to user u" || return $?
  test::ci::_etcdctl_expect "$endpoints""--user root:root role grant-permission r readwrite key1" "Role r updated" || return $?
  test::ci::_etcdctl_expect "$endpoints" "--user u:u put key1 value1" "OK" || return $?
  test::ci::_etcdctl_expect "$endpoints" "--user u:u get key1" "key1\nvalue1" || return $?
  test::ci::_etcdctl_expect "$endpoints" "--user u:u role get r" "Role r\nKV Read:\n\tkey1\nKV Write:\n\tkey1" || return $?
  test::ci::_etcdctl_expect "$endpoints" "--user u:u user get u" "User: u\nRoles: r" || return $?
  test::ci::_etcdctl_expect "$endpoints" "echo 'new_password' | --user root:root user passwd --interactive=false u" "Password updated" || return $?
  test::ci::_etcdctl_expect "$endpoints" "--user root:root role revoke-permission r key1" "Permission of key key1 is revoked from role r" || return $?
  test::ci::_etcdctl_expect "$endpoints" "--user root:root user revoke-role u r" "Role r is revoked from user u" || return $?
  test::ci::_etcdctl_expect "$endpoints" "--user root:root user list" "root\nu" || return $?
  test::ci::_etcdctl_expect "$endpoints" "--user root:root role list" "r\nroot" || return $?
  test::ci::_etcdctl_expect "$endpoints" "--user root:root user delete u" "User u deleted" || return $?
  test::ci::_etcdctl_expect "$endpoints" "--user root:root role delete r" "Role r deleted" || return $?
  test::ci::_etcdctl_match "$endpoints" "--user root:root user get non_exist_user" "etcdserver: user name not found" || return $?
  test::ci::_etcdctl_match "$endpoints" "--user root:root user add root:root" "etcdserver: user name already exists" || return $?
  test::ci::_etcdctl_match "$endpoints" "--user root:root role get non_exist_role" "etcdserver: role name not found" || return $?
  test::ci::_etcdctl_match "$endpoints" "--user root:root role add root" "etcdserver: role name already exists" || return $?
  test::ci::_etcdctl_match "$endpoints" "--user root:root user revoke root r" "etcdserver: role is not granted to the user" || return $?
  test::ci::_etcdctl_match "$endpoints" "--user root:root role revoke root non_exist_key" "etcdserver: permission is not granted to the role" || return $?
  test::ci::_etcdctl_match "$endpoints" "--user root:root user delete root" "etcdserver: invalid auth management" || return $?
  test::ci::_etcdctl_expect "$endpoints" "--user root:root auth disable" "Authentication Disabled" || return $?
  log::info "auth validation test passed"
}

function test::ci::_install_CRD() {
    pushd $(dirname "${BASH_SOURCE[0]}")/../../../
    make install
    popd
    if [ $? -eq 0 ]; then
        log::info "make install: create custom resource definition succeeded"
    else
        log::error "make install: create custom resource definition failed"
    fi
}

function test::ci::_uninstall_CRD() {
    pushd $(dirname "${BASH_SOURCE[0]}")/../../../
    make uninstall
    popd
    if [ $? -eq 0 ]; then
        log::info "make uninstall: remove custom resource definition succeeded"
    else
        log::error "make uninstall: remove custom resource definition failed"
    fi
}

function test::ci::wait_all_xline_pod_ready() {
  for ((i = 0; i < $1; i++)); do
    log::info "wait pod/${_TEST_CI_STS_NAME}-${i} to be ready"
    if ! k8s::kubectl wait --for=condition=Ready pod/${_TEST_CI_STS_NAME}-${i} --timeout=300s; then
      log::fatal "Failed to wait for util to be ready"
    fi
  done
}

function test::ci::wait_all_xline_pod_deleted() {
  for ((i = 0; i < $1; i++)); do
    log::info "wait pod/${_TEST_CI_STS_NAME}-${i} to be ready"
    if ! k8s::kubectl wait --for=delete pod/${_TEST_CI_STS_NAME}-${i} --timeout=300s; then
      log::fatal "Failed to wait for util to be ready"
    fi
  done
}

function test::ci::_prepare_pv() {
  log::info "create persistent volume and storage class"
  mkdir -p /tmp/host-500m-pv1 /tmp/host-500m-pv2 /tmp/host-500m-pv3
  k8s::kubectl apply -f "$(dirname "${BASH_SOURCE[0]}")/manifests/e2e-storage.yaml" >/dev/null 2>&1
  k8s::kubectl::wait_resource_creation storageclass $_TEST_CI_STORAGECLASS_NAME
  k8s::kubectl::wait_resource_creation pv "host-500m-pv1"
  k8s::kubectl::wait_resource_creation pv "host-500m-pv2"
  k8s::kubectl::wait_resource_creation pv "host-500m-pv3"
}

function test::ci::_clean_pvc() {
  for ((i = 0; i < $1; i++)); do
    local pvc_name="xline-storage-${_TEST_CI_STS_NAME}-${i}"
    log::info "deleting pvc $pvc_name ..."
    k8s::kubectl delete pvc $pvc_name >/dev/null 2>&1
    if ! k8s::kubectl wait --for=delete pvc/${pvc_name} --timeout=300s; then
      log::fatal "Failed to wait for pvc/${pvc_name} to be deleted"
    fi
  done
}

function test::ci::_clean_pv() {
  log::info "delete persistent volume claim"
  log::info "delete persistent volume and storage class"
  k8s::kubectl delete -f "$(dirname "${BASH_SOURCE[0]}")/manifests/e2e-storage.yaml"
  log::info "pv has been deleted"
  rm -rf /tmp/host-500m-pv1 /tmp/host-500m-pv2 /tmp/host-500m-pv3
}

function test::ci::_start() {
  test::ci::stop_controller
  test::ci::_install_CRD
  test::ci::start_controller
  log::info "create xline auth key pairs"
  k8s::kubectl apply -f "$(dirname "${BASH_SOURCE[0]}")/manifests/auth-cred.yaml" >/dev/null 2>&1
  k8s::kubectl::wait_resource_creation secret $_TEST_CI_SECRET_NAME
  test::ci::_prepare_pv
  log::info "starting xline cluster"
  k8s::kubectl apply -f "$(dirname "${BASH_SOURCE[0]}")/manifests/cluster.yaml" >/dev/null 2>&1
  k8s::kubectl::wait_resource_creation sts $_TEST_CI_STS_NAME
}

function test::ci::start_controller() {
    log::info "starting controller"
    pushd $(dirname "${BASH_SOURCE[0]}")/../../../
    make run >/dev/null 2>&1 &
    popd
    sleep 5
    if kill -0 $!; then
      log::info "Controller started"
    else
      log::error "Controller cannot failed"
    fi
}

function test::ci::stop_controller() {
  controller_pid=$(lsof -i:8081 | grep main | awk '{print $2}')
  if [ -n "$controller_pid" ]; then
    kill -9 $controller_pid
  fi
}


function test::ci::_teardown() {
  log::info "stopping controller"
  test::ci::_uninstall_CRD
  test::ci::stop_controller
  k8s::kubectl delete -f "$(dirname "${BASH_SOURCE[0]}")/manifests/cluster.yaml" >/dev/null 2>&1
  test::ci::wait_all_xline_pod_deleted 3
  test::ci::_clean_pvc 3
  test::ci::_clean_pv
  k8s::kubectl delete -f "$(dirname "${BASH_SOURCE[0]}")/manifests/auth-cred.yaml" >/dev/null 2>&1
}

function test::ci::_chaos() {
  size=$1
  iters=$2
  majority=$((size / 2 + 1))
  fault_tolerance=$((size - majority))
  log::info "chaos: size=$size, iters=$iters, fault_tolerance=$fault_tolerance"
  for ((i = 0; i < iters; i++)); do
    log::info "chaos: iter=$i"
    endpoints=$(test::ci::_mk_endpoints size)
    test::ci::_etcdctl_expect "$endpoints" "put A $i" "OK" || return $?
    test::ci::_etcdctl_expect "$endpoints" "get A" "A\n$i" || return $?
    kill=$((RANDOM % fault_tolerance + 1))
    log::info "chaos: kill=$kill"
    for ((j = 0; j < $kill; j++)); do
      pod="${_TEST_CI_STS_NAME}-$((RANDOM % size))"
      log::info "chaos: kill pod=$pod"
      k8s::kubectl delete pod "$pod" --force --grace-period=0 2>/dev/null
    done
    test::ci::_etcdctl_expect "$endpoints" "put B $i" "OK" || return $?
    test::ci::_etcdctl_expect "$endpoints" "get B" "B\n$i" || return $?
    k8s::kubectl wait --for=jsonpath='{.status.readyReplicas}'="$size" sts/$_TEST_CI_CLUSTER_NAME --timeout=300s >/dev/null 2>&1
    log::info "wait for log synchronization" && sleep $_TEST_CI_LOG_SYNC_TIMEOUT
  done
}

function test::run::ci::basic_validation() {
  test::ci::_start
  test::ci::wait_all_xline_pod_ready 3
  endpoints=$(test::ci::_mk_endpoints 3)
  test::ci::_etcdctl_expect "$endpoints" "put A 1" "OK" || return $?
  test::ci::_etcdctl_expect "$endpoints" "get A" "A\n1" || return $?
  endpoints=$(test::ci::_mk_endpoints 1)
  test::ci::_etcdctl_expect "$endpoints" "put A 2" "OK" || return $?
  test::ci::_etcdctl_expect "$endpoints" "get A" "A\n2" || return $?
  test::ci::_auth_validation
  test::ci::_teardown
}


function test::run::ci::basic_chaos() {
  test::ci::_start
  test::ci::_chaos 3 5 || return $?
  test::ci::_teardown
}
