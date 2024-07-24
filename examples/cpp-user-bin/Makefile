CC = cc
CFLAGS = -Wl,--no-as-needed -std=c++11 -pedantic -g -O0 -L../../target/release
LIBS=-lm -ldl -lpthread -l:libplugin_api.a -lstdc++

ODIR=./

bindings:
	sh ./bindgen.sh

%.o: %.cpp $(DEPS) bindings
	$(CC) -c -xc++ -o $@ $< $(CFLAGS)

main.out: main.o
	cargo build --release
	cp ../../target/release/libplugin_lib.so $(ODIR)/
	(cd ../cpp-plugin-lib/ && make)
	cp ../cpp-plugin-lib/libplugin_cpp.so $(ODIR)/
	$(CC) -o $@ $^ $(CFLAGS) $(LIBS)

.PHONY: all
all: main.out

.DEFAULT_GOAL := all

clean:
	rm -f libplugin_lib.so libplugin_cpp.so bindings.h $(ODIR)/*.o $(ODIR)/*.out
