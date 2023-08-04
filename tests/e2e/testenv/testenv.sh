${__E2E_TESTENV__:=false} && return 0 || __E2E_TESTENV__=true

source "$(dirname "${BASH_SOURCE[0]}")/k8s/kind.sh"
source "$(dirname "${BASH_SOURCE[0]}")/util/util.sh"
