#include <stdio.h>
#include <string.h>
#include "bindings.h"

// Retrieves this object inside of a CtxBox container
#define this(obj) (obj .instance.inner.instance)
// Retrieves the context inside of a CtxBox container
#define ctx(obj) (& obj .instance.ctx)

// Allows to safely call functions that take a single self parameter
#define this_call(obj, func, ...) obj . func (this(obj), ## __VA_ARGS__)
// Allows to safely call functions that take both the self parameter and the context
#define this_ctx_call(obj, func, ...) obj . func (this(obj), ctx(obj), ## __VA_ARGS__)

int trim(char *str);

template<typename T>
void use_kvstore(T& obj);

template<typename T>
void kvdump(T& obj);

int main() {
	char name[256];

	printf("Enter name of the plugin library [plugin_lib]:\n");

	fgets(name, sizeof(name), stdin);
	int len = trim(name);

	auto obj = load_plugin(len > 0 ? name : "plugin_lib");

	{
		auto borrowed = obj.vtbl->borrow_features(this(obj), ctx(obj));

		this_call(borrowed, vtbl_mainfeature->print_self);

		if (borrowed.vtbl_keyvaluestore != nullptr) {
			printf("Using borrowed kvstore:\n");
			use_kvstore(borrowed);
		}

		if (borrowed.vtbl_keyvaluedumper != nullptr) {
			printf("Dumping borrowed kvstore:\n");
			kvdump(borrowed);
		}

		printf("Borrowed done.\n");
	}

	{
		auto owned = obj.vtbl->into_features(obj.instance);

		this_call(owned, vtbl_mainfeature->print_self);

		if (owned.vtbl_keyvaluestore != nullptr) {
			printf("Using owned kvstore:\n");
			use_kvstore(owned);
		}

		if (owned.vtbl_keyvaluedumper != nullptr) {
			printf("Dumping owned kvstore:\n");
			kvdump(owned);
		}
	}

	return 0;
}

int trim(char *str) {
	int len = strlen(str);

	for (int i = 0; i < 2; i++) {
		char c = str[--len];

		if (c == '\n' || c == '\r') {
			str[len] = '\0';
		} else {
			len++;
			break;
		}
	}

	return len;
}

template<typename T>
void use_kvstore(T& obj) {
	char key[256];

	printf("Enter key:\n");

	fgets(key, sizeof(key), stdin);
	int len = trim(key);

	CSliceRef<unsigned char> key_slice;
	key_slice.data = (unsigned char *)key;
	key_slice.len = len;

	printf("Cur val: %zu\n", obj.vtbl_keyvaluestore->get_key_value(this(obj), key_slice));

	size_t new_val = 0;

	printf("Enter value:\n");
	scanf("%zu", &new_val);

	char nl[2];
	fgets(nl, sizeof(nl), stdin);

	this_call(obj, vtbl_keyvaluestore->write_key_value, key_slice, new_val);
}

bool kvdump_callback(void *, KeyValue kv) {
	fwrite(kv._0.data, sizeof(char), kv._0.len, stdout);
	printf(" : %zu\n", kv._1);
	return true;
}

template<typename T>
void kvdump(T& obj) {
	KeyValueCallback callback;
	callback.context = nullptr;
	callback.func = kvdump_callback;

	this_call(obj, vtbl_keyvaluedumper->dump_key_values, callback);
}

