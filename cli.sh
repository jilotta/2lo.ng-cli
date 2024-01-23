#!/bin/sh

set -fn

CMD="$1"
TWOLONG="http://localhost:8080"

if ! command -v curl > /dev/null; then
        echo "CURL not found." >&2
        exit 1
fi


if ! curl -s "$TWOLONG" >/dev/null; then
        echo "Offline / 2lo.ng down."
        exit 2
fi

print_response() {
        echo "  - $TWOLONG/$2"
        echo "  - $TWOLONG/.$1"
}

if [ "$CMD" = "stats" ]; then
        shift
        for i in "$@"; do
                click_count="$(curl -s "$TWOLONG/api/stats/$i")"
                if [ -z "$click_count" ]; then
                        echo "$i: not found"
                else
                        echo "$i: $click_count clicks"
                fi
        done
else
        DELIMITER="+"
        for i in "$@"; do
                destination=$(echo "$i" | cut -d "$DELIMITER" -f 1)
                id=$(echo "$i" | cut -d "$DELIMITER" -f 2)

                echo "$destination: "
                if [ "$destination" = "$id" ]; then
                        response="$(curl -s --data-urlencode "link=$destination" -X POST $TWOLONG/api/add)"
                        print_response $response
                else
                        response="$(curl -s --data-urlencode "link=$destination" -X POST $TWOLONG/api/add/"$id")"
                        if [ -z "$response" ]; then
                                echo "  - ID \`$id\` already taken!"
				exit 3
                        else
                                print_response $response
                        fi
                fi
        done
fi

