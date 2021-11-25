#include <stdio.h>
#include <string.h>
#include "bindings.h"

int trim(char *str);

void use_kvstore(FeaturesGroup *obj);
void kvdump(FeaturesGroup *obj);

int main() {
	char name[256];

	printf("Enter name of the plugin library [plugin_lib]:\n");

	fgets(name, sizeof(name), stdin);
	int len = trim(name);

	PluginInner obj;
	int ret;

	if (ret = load_plugin(len > 0 ? name : "plugin_lib", &obj)) {
		printf("Failed to load plugin (%d)!\n", ret);
		plugininner_drop(obj);
		return 1;
	}

	{
		FeaturesGroup borrowed = borrow_features(&obj);

		featuresgroup_print_self(&borrowed);

		if (borrowed.vtbl_keyvaluestore != NULL) {
			printf("Using borrowed kvstore:\n");
			use_kvstore(&borrowed);
		}

		if (borrowed.vtbl_keyvaluedumper != NULL) {
			printf("Dumping borrowed kvstore:\n");
			kvdump(&borrowed);
		}

		printf("Borrowed done.\n");

		featuresgroup_drop(borrowed);
	}

	{
		FeaturesGroup owned = into_features(obj);

		featuresgroup_print_self(&owned);

		if (owned.vtbl_keyvaluestore != NULL) {
			printf("Using owned kvstore:\n");
			use_kvstore(&owned);
		}

		if (owned.vtbl_keyvaluedumper != NULL) {
			printf("Dumping owned kvstore:\n");
			kvdump(&owned);
		}

		featuresgroup_drop(owned);
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

void use_kvstore(FeaturesGroup *obj) {
	char key[256];

	printf("Enter key:\n");

	fgets(key, sizeof(key), stdin);
	int len = trim(key);

	CSliceRef_u8 key_slice;
	key_slice.data = (unsigned char *)key;
	key_slice.len = len;

	printf("Cur val: %zu\n", featuresgroup_get_key_value(obj, key_slice));

	size_t new_val = 0;

	printf("Enter value:\n");
	scanf("%zu", &new_val);

	char nl[2];
	fgets(nl, sizeof(nl), stdin);

	featuresgroup_write_key_value(obj, key_slice, new_val);
}

bool kvdump_callback(void *unused, KeyValue kv) {
	fwrite(kv._0.data, sizeof(char), kv._0.len, stdout);
	printf(" : %zu\n", kv._1);
	return true;
}

void kvdump(FeaturesGroup *obj) {
	featuresgroup_dump_key_values(obj, CALLBACK(KeyValue, NULL, kvdump_callback));

	int ints[32];

	for (int i = 0; i < 32; i++) {
		ints[i] = i * i;
	}

	BUF_ITER_ARR_SPEC(i32, int, int_iter, ints);

	featuresgroup_print_ints(obj, int_iter);
}

