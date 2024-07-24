CC = cc
CFLAGS = -pedantic -Wl,--no-as-needed -std=c++14 -g -O0 -shared -fpie -fPIC -L../../target/release
LIBS=-lm -ldl -lpthread -l:libplugin_api.a -lstdc++

ODIR=./

bindings:
	sh ./bindgen.sh

%.o: %.cpp $(DEPS) bindings
	$(CC) -c -xc++ -o $@ $< $(CFLAGS)

libplugin_cpp.so: main.o
	cargo build --release
	$(CC) -o $@ $^ $(CFLAGS) $(LIBS)
	cp libplugin_cpp.so ../../target/release/
	cp libplugin_cpp.so ../../target/debug/

.PHONY: all
all: libplugin_cpp.so

.DEFAULT_GOAL := all

clean:
	rm -f bindings.h $(ODIR)/*.o $(ODIR)/*.so
