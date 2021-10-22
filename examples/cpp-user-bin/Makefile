CC = cc
CFLAGS = -g -O0 -L../../target/debug
LIBS=-lm -ldl -lpthread -l:libplugin_api.a -lstdc++

ODIR=./

bindings:
	sh ./bindgen.sh

%.o: %.cpp $(DEPS) #bindings
	$(CC) -c -o $@ $< $(CFLAGS)

main.out: main.o
	cargo build --release
	cp ../../target/debug/libplugin_lib.so $(ODIR)/
	$(CC) -o $@ $^ $(CFLAGS) $(LIBS)

.PHONY: all
all: main.out

.DEFAULT_GOAL := all

clean:
	rm -f libplugin_lib.so bindings.h $(ODIR)/*.o $(ODIR)/*.out
