// This example shows client-side usage from C.

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

	// Load the plugin, returns 0 on success.
	// Otherwise, obj has undefined data.
	if (ret = load_plugin(len > 0 ? name : "plugin_lib", &obj)) {
		printf("Failed to load plugin (%d)!\n", ret);
		plugininner_drop(obj);
		return 1;
	}

	// In this block we temporarily borrow the features.
	{
		// Until `borrowed` is dropped, `obj` should not be touched.
		FeaturesGroup borrowed = borrow_features(&obj);

		// print_self is part of MainFeature, and always exists.
		featuresgroup_print_self(&borrowed);

		// These are optional features we check to see if they exist
		if (borrowed.vtbl_keyvaluestore != NULL) {
			printf("Using borrowed kvstore:\n");
			use_kvstore(&borrowed);
		}

		if (borrowed.vtbl_keyvaluedumper != NULL) {
			printf("Dumping borrowed kvstore:\n");
			kvdump(&borrowed);
		}

		printf("Borrowed done.\n");

		// Release the resources, `obj` is safe to use again.
		featuresgroup_drop(borrowed);
	}

	// In this block we consume obj and it becomes unusable.
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

		// We drop the last CGlue object, meaning the library should be freed.
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
	trim(key);

	// STR will automatically construct a CSliceRef,
	// but it can also be done manually for reuse.
	printf("Cur val: %zu\n", featuresgroup_get_key_value(obj, STR(key)));

	size_t new_val = 0;

	printf("Enter value:\n");
	scanf("%zu", &new_val);

	char nl[2];
	fgets(nl, sizeof(nl), stdin);

	featuresgroup_write_key_value(obj, STR(key), new_val);
}

bool kvdump_callback(void *unused, KeyValue kv) {
	fwrite(kv._0.data, sizeof(char), kv._0.len, stdout);
	printf(" : %zu\n", kv._1);
	return true;
}

void kvdump(FeaturesGroup *obj) {
	// Construct the simplest callback here that takes in the `kvdump_callback` function.
	featuresgroup_dump_key_values(obj, CALLBACK(KeyValue, NULL, kvdump_callback));

	int ints[32];

	for (int i = 0; i < 32; i++) {
		ints[i] = i * i;
	}

	// This macro takes the statically known array size,
	// and construct a CIterator with the buffer.
	// i32 is the rust type for int, thus we have to
	// specify the type. For structure types, this is
	// usually not needed, and we can use `BUF_ITER_ARR`.
	BUF_ITER_ARR_SPEC(i32, int, int_iter, ints);

	featuresgroup_print_ints(obj, int_iter);
}

