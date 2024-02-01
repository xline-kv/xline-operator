package v1alpha1

import (
	"github.com/stretchr/testify/assert"
	"testing"
)

func TestXlineClusterSpec_BootArgs(t *testing.T) {
	curpDir := "curp"
	tests := []struct {
		name   string
		fields XlineArgs
		want   []string
	}{
		{
			name: "case 1",
			fields: XlineArgs{
				ClientUseBackoff: true,
				RetryCount:       5,
				CmdWorkers:       8,
			},
			want: []string{"--client-use-backoff", "--retry-count", "5", "--cmd-workers", "8"},
		},
		{
			name: "case 2",
			fields: XlineArgs{
				CurpDir: &curpDir,
			},
			want: []string{"--curp-dir", curpDir},
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			s := &XlineClusterSpec{
				BootstrapArgs: tt.fields,
			}
			assert.Equalf(t, tt.want, s.BootArgs(), "BootArgs()")
		})
	}
}
