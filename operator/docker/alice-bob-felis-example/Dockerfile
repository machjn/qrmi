# Example workload for the QRMI operator: runs a QIR program on Alice & Bob Felis,
# using qrmi from PyPI.
#
# The program takes everything from the environment a scheduler gives it, as a
# Slurm job does under the QRMI SPANK plugin: the operator's per-Job Secret sets
# QRMI_JOB_QPU_RESOURCES, QRMI_JOB_QPU_TYPES and the <resource>_QRMI_* variables.
#
# Usage (from the repository root):
#   docker build -f operator/docker/alice-bob-felis-example/Dockerfile -t felis-qrmi-example:latest .
#   docker run --rm \
#     -e QRMI_JOB_QPU_RESOURCES=ab_emu_1q_lescanne_2020 \
#     -e QRMI_JOB_QPU_TYPES=alice-bob-felis \
#     -e ab_emu_1q_lescanne_2020_QRMI_AB_FELIS_BASE_ENDPOINT=https://api.alice-bob.com \
#     -e ab_emu_1q_lescanne_2020_QRMI_AB_FELIS_API_KEY=<key> \
#     felis-qrmi-example:latest

FROM python:3.12-slim

WORKDIR /app

RUN pip install --no-cache-dir qrmi==0.26.3

COPY operator/docker/alice-bob-felis-example/example.py ./example.py
COPY operator/generated_circuit.ll ./generated_circuit.ll

ENTRYPOINT ["python", "example.py", "generated_circuit.ll"]
