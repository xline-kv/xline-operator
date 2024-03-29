package controller

import (
	"fmt"
	"strings"

	"k8s.io/apimachinery/pkg/api/errors"
	ctrl "sigs.k8s.io/controller-runtime"
)

// StCtrlErrSet is the standard controller error container
type StCtrlErrSet struct {
	Rec    error
	Sync   error
	Update error
}

func (r *StCtrlErrSet) AsResult() (ctrl.Result, error) {
	// Silent update conflict error
	updateConflict := false
	if r.Update != nil && errors.IsConflict(r.Update) {
		r.Update = nil
		updateConflict = true
	}
	errMap := make(map[string]error)
	if r.Rec != nil {
		errMap["rec"] = r.Rec
	}
	if r.Sync != nil {
		errMap["sync"] = r.Sync
	}
	if r.Update != nil {
		errMap["update-status"] = r.Update
	}
	if len(errMap) == 0 {
		if updateConflict {
			return ctrl.Result{Requeue: true}, nil
		} else {
			return ctrl.Result{}, nil
		}
	} else {
		return ctrl.Result{Requeue: true}, &MultiTaggedError{Errors: errMap}
	}
}

// MultiTaggedError is a list of errors with tags.
type MultiTaggedError struct {
	Errors map[string]error
}

func (e *MultiTaggedError) Error() string {
	errStrs := make([]string, 0, len(e.Errors))
	for tag, err := range e.Errors {
		errStrs = append(errStrs, fmt.Sprintf("[%s] %s", tag, err.Error()))
	}
	return strings.Join(errStrs, "; ")
}
