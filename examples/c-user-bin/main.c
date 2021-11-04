#include <stdio.h>
#include <string.h>
#include "bindings.h"


int trim(char *str);

void use_kvstore(FeaturesGroupArcBox *obj);
void kvdump(FeaturesGroupArcBox *obj);

// Retrieves this object inside of a CtxBox container
#define this(obj) (&((obj) ->container))

// Allows to safely call functions that take a single self parameter
#define this_call(obj, func, ...) (obj) -> func (this(obj), ## __VA_ARGS__)
#define this_owned_call(obj, func, ...) (obj) -> func (*this(obj), ## __VA_ARGS__)

#define drop_arc(obj) (obj).container.context.drop_fn((obj).container.context.instance) \

#define drop_box(obj) { \
	drop_arc(obj); \
	(obj).container.instance.drop_fn((obj).container.instance.instance); \
}

int main() {
	char name[256];

	printf("Enter name of the plugin library [plugin_lib]:\n");

	fgets(name, sizeof(name), stdin);
	int len = trim(name);

	PluginInnerArcBox obj = load_plugin(len > 0 ? name : "plugin_lib");

	{
		FeaturesGroupArcBox borrowed = borrow_features(&obj);

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

		featuresgroup_arc_box_drop(borrowed);
	}

	{
		FeaturesGroupArcBox owned = arc_box_into_features(obj);

		featuresgroup_print_self(&owned);

		if (owned.vtbl_keyvaluestore != NULL) {
			printf("Using owned kvstore:\n");
			use_kvstore(&owned);
		}

		if (owned.vtbl_keyvaluedumper != NULL) {
			printf("Dumping owned kvstore:\n");
			kvdump(&owned);
		}

		featuresgroup_arc_box_drop(owned);
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

void use_kvstore(FeaturesGroupArcBox *obj) {
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

bool kvdump_callback(void *, KeyValue kv) {
	fwrite(kv._0.data, sizeof(char), kv._0.len, stdout);
	printf(" : %zu\n", kv._1);
	return true;
}

void kvdump(FeaturesGroupArcBox *obj) {
	KeyValueCallback callback;
	callback.context = NULL;
	callback.func = kvdump_callback;

	featuresgroup_dump_key_values(obj, callback);
}

