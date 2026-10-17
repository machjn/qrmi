# SPDX-License-Identifier: Apache-2.0

# Local development against a kind cluster (Podman provider).
#
#   make kind-up    create the cluster (if needed), build + load the image,
#                   and deploy CRDs, RBAC and the operator
#   make reload     rebuild the image, load it into the cluster and restart
#                   the operator
#   make kind-down  delete the cluster
#
#   make example    build + load the Alice & Bob Felis example image, create
#                   the credentials secret and QuantumResource, then run the
#                   example Job and print its output
#                   (needs QRMI_AB_FELIS_API_KEY on first run)

SHELL := /bin/bash

ROOT_DIR       := $(abspath $(CURDIR)/..)
CLUSTER        ?= qrmi
KUBE_CONTEXT   := kind-$(CLUSTER)
# Must match the namespace hardcoded in deploy/operator.yaml and deploy/rbac/rbac.yaml.
NAMESPACE      := platform-qrmi
CONTAINER_TOOL ?= podman
# Images are fully qualified so they match the unqualified `image:` names in the
# manifests (podman would otherwise tag them localhost/<name>).
IMAGE          ?= docker.io/library/qrmi-operator:latest
EXAMPLE_IMAGE  ?= docker.io/library/felis-qrmi-example:latest
ARCHIVE_DIR    := $(ROOT_DIR)/target/kind

# Names used by the manifests in deploy/resources/.
EXAMPLE_JOB     := alice-bob-felis-example
EXAMPLE_SECRET  := alice-bob-felis-credentials
EXAMPLE_TIMEOUT ?= 300

export KIND_EXPERIMENTAL_PROVIDER ?= podman

KUBECTL := kubectl --context $(KUBE_CONTEXT)

# `kind load docker-image` can't find docker.io/library/* images under podman,
# so go through an image archive instead.
# $(1): image, $(2): archive basename
define load_image
	@mkdir -p $(ARCHIVE_DIR)
	rm -f $(ARCHIVE_DIR)/$(2).tar
	$(CONTAINER_TOOL) save -o $(ARCHIVE_DIR)/$(2).tar $(1)
	kind load image-archive $(ARCHIVE_DIR)/$(2).tar --name $(CLUSTER)
endef

.PHONY: kind-up reload kind-down cluster image load deploy restart logs
.PHONY: example example-image example-load example-secret example-resource
.PHONY: example-run example-logs

kind-up: cluster image load deploy

reload: image load restart

kind-down:
	kind delete cluster --name $(CLUSTER)

cluster:
	@if kind get clusters 2>/dev/null | grep -qx '$(CLUSTER)'; then \
		echo "kind cluster '$(CLUSTER)' already exists"; \
	else \
		kind create cluster --name $(CLUSTER); \
	fi

image:
	$(CONTAINER_TOOL) build -f $(ROOT_DIR)/operator/docker/operator/Dockerfile -t $(IMAGE) $(ROOT_DIR)

load:
	$(call load_image,$(IMAGE),qrmi-operator)

deploy:
	$(KUBECTL) create namespace $(NAMESPACE) --dry-run=client -o yaml | $(KUBECTL) apply -f -
	$(KUBECTL) apply -f deploy/crds/crds.yaml
	$(KUBECTL) apply -f deploy/rbac/rbac.yaml
	$(KUBECTL) apply -f deploy/operator.yaml
	$(KUBECTL) -n $(NAMESPACE) rollout status deploy/qrmi-operator --timeout=120s

restart:
	$(KUBECTL) -n $(NAMESPACE) rollout restart deploy/qrmi-operator
	$(KUBECTL) -n $(NAMESPACE) rollout status deploy/qrmi-operator --timeout=120s

logs:
	$(KUBECTL) -n $(NAMESPACE) logs -f deploy/qrmi-operator

# ------------------------------------------------
# Alice & Bob Felis example payload
# ------------------------------------------------

example: example-image example-load example-secret example-resource example-run

example-image:
	$(CONTAINER_TOOL) build -f $(ROOT_DIR)/operator/docker/alice-bob-felis-example/Dockerfile -t $(EXAMPLE_IMAGE) $(ROOT_DIR)

example-load:
	$(call load_image,$(EXAMPLE_IMAGE),felis-qrmi-example)

# Creates/updates the secret from QRMI_AB_FELIS_API_KEY. If the variable is unset,
# an existing secret is kept so later runs don't need the key exported.
example-secret:
	@if [ -n "$$QRMI_AB_FELIS_API_KEY" ]; then \
		$(KUBECTL) -n $(NAMESPACE) create secret generic $(EXAMPLE_SECRET) \
			--from-literal=api-key="$$QRMI_AB_FELIS_API_KEY" --dry-run=client -o yaml \
			| $(KUBECTL) apply -f -; \
	elif $(KUBECTL) -n $(NAMESPACE) get secret $(EXAMPLE_SECRET) >/dev/null 2>&1; then \
		echo "QRMI_AB_FELIS_API_KEY not set, using existing secret $(EXAMPLE_SECRET)"; \
	else \
		echo "QRMI_AB_FELIS_API_KEY is not set and secret $(EXAMPLE_SECRET) does not exist" >&2; \
		exit 1; \
	fi

# QuantumResource is cluster-scoped, so no namespace.
example-resource:
	$(KUBECTL) apply -f deploy/resources/alice-bob-felis-qr.yaml

# Jobs are immutable, so delete any previous run before re-creating it, then
# wait for it to complete or fail and print its output.
example-run:
	$(KUBECTL) -n $(NAMESPACE) delete job $(EXAMPLE_JOB) --ignore-not-found --wait
	$(KUBECTL) -n $(NAMESPACE) apply -f deploy/resources/alice-bob-felis-job-annotation.yaml
	@echo "waiting up to $(EXAMPLE_TIMEOUT)s for job/$(EXAMPLE_JOB)..."
	@for ((i = 0; i < $(EXAMPLE_TIMEOUT); i += 2)); do \
		state=$$($(KUBECTL) -n $(NAMESPACE) get job $(EXAMPLE_JOB) \
			-o jsonpath='{.status.conditions[?(@.status=="True")].type}'); \
		case "$$state" in \
			*Complete*) $(KUBECTL) -n $(NAMESPACE) logs job/$(EXAMPLE_JOB); exit 0 ;; \
			*Failed*) $(KUBECTL) -n $(NAMESPACE) logs job/$(EXAMPLE_JOB); \
				echo "job/$(EXAMPLE_JOB) failed" >&2; exit 1 ;; \
		esac; \
		sleep 2; \
	done; \
	echo "timed out waiting for job/$(EXAMPLE_JOB)" >&2; \
	$(KUBECTL) -n $(NAMESPACE) describe job $(EXAMPLE_JOB) | tail -20; \
	exit 1

example-logs:
	$(KUBECTL) -n $(NAMESPACE) logs job/$(EXAMPLE_JOB)
