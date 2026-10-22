# -*- coding: utf-8 -*-

# This code is part of Qiskit.
#
# (C) Copyright 2026 Alice and Bob. All Rights Reserved.
#
# This code is licensed under the Apache License, Version 2.0. You may
# obtain a copy of this license in the LICENSE.txt file in the root directory
# of this source tree or at http://www.apache.org/licenses/LICENSE-2.0.
#
# Any modifications or derivative works of this code must retain this
# copyright notice, and modified files need to carry a notice indicating
# that they have been altered from the originals.

"""Run a QIR program on the quantum resource the scheduler allocated to this job.

The program finds its resource the way a Slurm job does, from
QRMI_JOB_QPU_RESOURCES and the per-resource variables, and never acquires or
releases it: the scheduler, here the QRMI operator, does that around the job.
"""

import argparse
import json
import os
import sys
import time

from qrmi import Payload, QRMIService, TaskStatus


def print_qrmi_variables():
    """Print the names, never the values, of the QRMI variables this job was given."""
    names = sorted(
        name
        for name in os.environ
        if name.startswith(("QRMI_", "SLURM_JOB_QPU_")) or "_QRMI_" in name
    )
    print("QRMI variables:", ", ".join(names) or "none", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("qir_file", help="QIR program (.ll)")
    parser.add_argument("--shots", type=int, default=100)
    args = parser.parse_args()

    print_qrmi_variables()
    resources = QRMIService().resources()
    if len(resources) != 1:
        sys.exit(f"expected one quantum resource for this job, got {len(resources)}")
    resource = resources[0]

    with open(args.qir_file, encoding="utf-8") as f:
        payload = Payload.AliceBobFelis(
            human_qir=f.read(), input_params=json.dumps({"nbShots": args.shots})
        )
    task = resource.task_start(payload)
    print(f"Resource: {resource.resource_id()}; task: {task}", flush=True)

    while (status := resource.task_status(task)) in (TaskStatus.Queued, TaskStatus.Running):
        time.sleep(1)
    if status != TaskStatus.Completed:
        sys.exit(f"task {task} ended {status}")
    print(resource.task_result(task).value)


if __name__ == "__main__":
    main()
