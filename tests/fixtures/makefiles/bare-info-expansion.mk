# GNU Make 4.4.1 expands $(info ...), $(warning ...) and $(error ...) to
# empty text, so these lines define nothing and the parse stays complete.
NAME := demo
$(info building $(NAME))
$(warning NAME is fixed) # a trailing comment
ifndef NAME
$(error NAME must be set)
endif

build:
	@echo $(NAME)
