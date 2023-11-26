${__E2E_COMMON__:=false} && return 0 || __E2E_COMMON__=true

source "$(dirname "${BASH_SOURCE[0]}")/log.sh"
source "$(dirname "${BASH_SOURCE[0]}")/k8s.sh"
