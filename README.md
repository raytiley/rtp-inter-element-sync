# AV Sync Issue Reproduction

This repository reproduces an audio avsync issue when using intersink and intersrc elements. It starts an rtp stream  and then creates a reciving pipeline that sends the raw audio and video to an intersink elements.

Five seconds later it will start to output that pipeline to autovideo and autoaudio sinks using intersrc elements. The Audio and video will be out of sync.

To test locally adjust the `uri` path in the `main` method.
