#version 450
// GELU activation (approximate): 0.5 * x * (1 + tanh(sqrt(2/pi) * (x + 0.044715 * x^3)))

uniform sampler2D u_input;
out float frag_output;

void main() {
    float x = texelFetch(u_input, ivec2(gl_FragCoord.xy), 0).r;
    
    float c1 = 0.7978845608; // sqrt(2/pi)
    float c2 = 0.044715;
    
    float x3 = x * x * x;
    frag_output = 0.5 * x * (1.0 + tanh(c1 * (x + c2 * x3)));
}
