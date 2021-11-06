#include <stdio.h>
#include <string.h>
#include "bindings.h"
#include <vector>

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
		printf("%p %p\n", obj.container.instance.instance, obj.container.instance.drop_fn);
		auto borrowed = obj.borrow_features();
		printf("%p %p\n", borrowed.container.instance.instance, borrowed.container.instance.drop_fn);

		borrowed.print_self();

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
		printf("%p %p\n", obj.container.instance.instance, obj.container.instance.drop_fn);
		auto owned = std::move(obj).into_features();


		owned.print_self();

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

	printf("Cur val: %zu\n", obj.get_key_value(key_slice));

	size_t new_val = 0;

	printf("Enter value:\n");
	scanf("%zu", &new_val);

	char nl[2];
	fgets(nl, sizeof(nl), stdin);

	obj.write_key_value(key_slice, new_val);
}

template<typename T>
void kvdump(T& obj) {
	obj.dump_key_values([](KeyValue kv) {
		fwrite(kv._0.data, sizeof(char), kv._0.len, stdout);
		printf(" : %zu\n", kv._1);
		return true;
	});

	std::vector<int> ints;

	for (int i = 0; i < 32; i++) {
		ints.push_back(i * i);
	}

	obj.print_ints(CPPIterator<std::vector<int>>(ints));
}

