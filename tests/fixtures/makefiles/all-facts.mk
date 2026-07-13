MODE ?= debug
include $(CONFIG_DIR)/common.mk

ifdef CI
check:: prepare
	@-+cargo test
else
check: local
	echo local
endif
