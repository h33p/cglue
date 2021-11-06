CC = cc
CFLAGS = -Wl,--no-as-needed -g -O0 -std=c99 -L../../target/release
LIBS=-lm -ldl -lpthread -l:libplugin_api.a

ODIR=./

bindings:
	sh ./bindgen.sh

%.o: %.c $(DEPS) bindings
	$(CC) -c -o $@ $< $(CFLAGS)

main.out: main.o
	cargo build --release
	cp ../../target/release/libplugin_lib.so $(ODIR)/
	$(CC) -o $@ $^ $(CFLAGS) $(LIBS)

.PHONY: all
all: main.out

.DEFAULT_GOAL := all

clean:
	rm -f libplugin_lib.so bindings.h $(ODIR)/*.o $(ODIR)/*.out
