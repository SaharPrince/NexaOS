/* NexaOS-owned LLVM/LLD PE32+ compatibility fixture source. */
volatile unsigned char initialized_payload[4096] = {
    0x5a, 0xa5, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66
};
volatile unsigned char zero_initialized_payload[16384];

/* A one-byte section checks raw file-alignment padding from a second linker. */
#pragma section(".nexa_pad", read, write)
__declspec(allocate(".nexa_pad")) volatile unsigned char raw_padding_probe = 0x7b;

void entry(void) {
    initialized_payload[0] = (unsigned char)(initialized_payload[0] + 1);
    zero_initialized_payload[0] = initialized_payload[0];
    raw_padding_probe ^= zero_initialized_payload[0];
}
