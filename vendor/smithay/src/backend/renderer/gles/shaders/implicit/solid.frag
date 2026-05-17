#version 100

precision mediump float;
uniform vec4 color;

void main() {
    // A19.4: pow gamma em demultiplied (centro vs borda AA mesma cor)
    vec3 srgb_rgb;
    if (color.a > 0.0001) {
        vec3 lin = color.rgb / color.a;
        srgb_rgb = pow(lin, vec3(1.0/2.2)) * color.a;
    } else {
        srgb_rgb = vec3(0.0);
    }
    gl_FragColor = vec4(srgb_rgb, color.a);
}