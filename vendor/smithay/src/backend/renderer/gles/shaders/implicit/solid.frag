#version 100

precision mediump float;
uniform vec4 color;

void main() {
    // A17.2: linear -> sRGB encode no output
    vec3 srgb_rgb = pow(color.rgb, vec3(1.0/2.2));
    gl_FragColor = vec4(srgb_rgb, color.a);
}