#include "bindings.h"

void print_version()
{
    CedVersion *version = ced_version();
    if (version->pre != NULL && strlen(version->pre) > 0)
    {
        printf("version: %s.%s.%s-%s\n", version->major, version->minor, version->patch, version->pre);
    }
    else
    {
        printf("version: %s.%s.%s\n", version->major, version->minor, version->patch);
    }
    ced_version_destroy(version);
}

int main(int argc, char *argv[])
{
    print_version();

    char *session = "ffi";
    CedConnection *conn = ced_connection_create(session);
    CedEvent *ev;
    while ((ev = ced_connection_next_event(conn)) != NULL)
    {
        printf("type: %d\n", ev->tag);
        if (ev->tag == CED_EVENT_INFO)
        {
            printf("info: %s %s\n", ev->INFO.client, ev->INFO.session);
        }
        else if (ev->tag == CED_EVENT_STATUS)
        {
            printf("status:\n");
            StatusItem *item;
            while ((item = ced_status_next_item(ev->STATUS.items)) != NULL)
            {
                printf("    (%d) %p\n", item->index, item->text);
                ced_status_item_destroy(item);
            }
        }
        ced_event_destroy(ev);
    }
    ced_connection_destroy(conn);
    puts("OUT");
    return 0;
}