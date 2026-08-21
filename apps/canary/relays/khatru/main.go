// Command khatru-lab-relay runs a second third-party relay implementation for
// the Fava canary. The relay behavior belongs to khatru and eventstore; this
// file only selects the configuration the evidence lab requires.
package main

import (
	"context"
	"flag"
	"fmt"
	"net/http"
	"os"
	"sync"

	"github.com/fiatjaf/eventstore/slicestore"
	"github.com/fiatjaf/khatru"
	"github.com/nbd-wtf/go-nostr"
	"github.com/nbd-wtf/go-nostr/nip11"
)

func main() {
	port := flag.Int("port", 0, "loopback TCP port to listen on")
	name := flag.String("name", "fava-canary-khatru", "relay name")
	maxSubscriptions := flag.Int("max-subscriptions", 0, "advertised and enforced NIP-11 max_subscriptions")
	maxMessageLength := flag.Int("max-message-length", 0, "advertised NIP-11 max_message_length")
	maxLimit := flag.Int("max-limit", 0, "advertised NIP-11 max_limit")
	maxSubidLength := flag.Int("max-subid-length", 0, "advertised NIP-11 max_subid_length")
	requireAuth := flag.Bool("require-auth", false, "require NIP-42 authentication before accepting an EVENT")
	flag.Parse()
	if *port == 0 {
		fmt.Fprintln(os.Stderr, "khatru-lab-relay requires --port")
		os.Exit(2)
	}

	relay := khatru.NewRelay()
	relay.Info.Name = *name
	relay.Info.Description = "Second relay implementation for the Fava evidence lab"
	relay.Info.Software = "https://github.com/fiatjaf/khatru"
	relay.Info.Limitation = &nip11.RelayLimitationDocument{
		MaxSubscriptions: *maxSubscriptions,
		MaxMessageLength: *maxMessageLength,
		MaxLimit:         *maxLimit,
		MaxSubidLength:   *maxSubidLength,
		AuthRequired:     *requireAuth,
	}

	store := &slicestore.SliceStore{}
	if err := store.Init(); err != nil {
		fmt.Fprintln(os.Stderr, "store init failed:", err)
		os.Exit(1)
	}
	relay.StoreEvent = append(relay.StoreEvent, store.SaveEvent)
	relay.QueryEvents = append(relay.QueryEvents, store.QueryEvents)
	relay.DeleteEvent = append(relay.DeleteEvent, store.DeleteEvent)

	if *requireAuth {
		relay.RejectEvent = append(relay.RejectEvent,
			func(ctx context.Context, event *nostr.Event) (bool, string) {
				if khatru.GetAuthed(ctx) == "" {
					khatru.RequestAuth(ctx)
					return true, "auth-required: publish requires NIP-42 authentication"
				}
				return false, ""
			})
	}
	if *maxSubscriptions > 0 {
		var open sync.Map // *khatru.WebSocket -> map[string]struct{}
		relay.OnDisconnect = append(relay.OnDisconnect, func(ctx context.Context) {
			open.Delete(khatru.GetConnection(ctx))
		})
		relay.RejectFilter = append(relay.RejectFilter,
			func(ctx context.Context, filter nostr.Filter) (bool, string) {
				connection := khatru.GetConnection(ctx)
				value, _ := open.LoadOrStore(connection, &subscriptionSet{ids: map[string]struct{}{}})
				set := value.(*subscriptionSet)
				id := khatru.GetSubscriptionID(ctx)
				set.mutex.Lock()
				defer set.mutex.Unlock()
				if _, known := set.ids[id]; known {
					return false, ""
				}
				if len(set.ids) >= *maxSubscriptions {
					return true, fmt.Sprintf("blocked: max_subscriptions is %d", *maxSubscriptions)
				}
				set.ids[id] = struct{}{}
				return false, ""
			})
	}

	address := fmt.Sprintf("127.0.0.1:%d", *port)
	fmt.Fprintf(os.Stdout, "khatru-lab-relay listening on ws://%s\n", address)
	if err := http.ListenAndServe(address, relay); err != nil {
		fmt.Fprintln(os.Stderr, "listen failed:", err)
		os.Exit(1)
	}
}

type subscriptionSet struct {
	mutex sync.Mutex
	ids   map[string]struct{}
}
