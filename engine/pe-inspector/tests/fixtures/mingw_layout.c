/* NexaOS-owned compatibility fixture source; generated PE is a CI artifact. */
volatile unsigned char initialized_payload[4096] = {
    0x5a, 0xa5, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66
};
volatile unsigned char zero_initialized_payload[16384];

int main(void) {
    initialized_payload[0] = (unsigned char)(initialized_payload[0] + 1);
    zero_initialized_payload[0] = initialized_payload[0];
    return (int)(initialized_payload[0] ^ zero_initialized_payload[0]);
}
