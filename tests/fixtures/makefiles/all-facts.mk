MODE ?= debug
export RELEASE = yes
override TOOL := cargo
define SCRIPT
echo configured
endef
include $(CONFIG_DIR)/common.mk

ifdef CI
check:: prepare
	@-+cargo test
else
check: local
	echo local
endif
