#include "http_hostcalls.h"
#include <assert.h>
#include <stdlib.h>
#include <string.h>

__attribute__((visibility("default"))) void run(void)
{
    char              response_body[] = "Hello, world!";
    enum value_status resp_body_stat =
        hostcall_resp_set_body(RESPONSE_OUTGOING, response_body, strlen(response_body));
    assert(resp_body_stat == value_status_ok);

    enum value_status resp_code_stat = hostcall_resp_set_response_code(RESPONSE_OUTGOING, 200);
    assert(resp_code_stat == value_status_ok);
}
