package reconciler

import (
	"errors"
	"testing"

	"github.com/stretchr/testify/assert"
	xapi "github.com/xline-kv/xline-operator/api/v1alpha1"
)

func TestStatusConvert(t *testing.T) {
	sucStage := clusterStageSucc(xapi.StageXlineService)
	failStage := clusterStageFail(xapi.StageXlineService, errors.New("failed to create service"))

	t.Run("Successful Status can covert to XlineClusterRecStatus properly", func(t *testing.T) {
		res := sucStage.AsXlineClusterRecStatus()
		assert.Equal(t, res.Stage, xapi.StageXlineService)
		assert.Equal(t, res.StageStatus, xapi.StageResultSucceeded)
		assert.True(t, res.LastMessage == "")
	})

	t.Run("Failed Status can covert to XlineClusterRecStatus properly", func(t *testing.T) {
		res := failStage.AsXlineClusterRecStatus()
		assert.Equal(t, res.Stage, xapi.StageXlineService)
		assert.Equal(t, res.StageStatus, xapi.StageResultFailed)
		assert.Equal(t, res.LastMessage, "failed to create service")
	})

}
