CC = cc
CFLAGS = -ansi -pedantic

.PHONY: all
all: fib

fib: src/main.c src/report.c src/fib.h
	$(CC) $(CFLAGS) -o fib src/main.c src/report.c
