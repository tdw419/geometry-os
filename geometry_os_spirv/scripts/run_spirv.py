#!/usr/bin/env python3
"""
Minimal Vulkan Compute Shader Runner for Geometry OS SPIR-V

Executes a SPIR-V compute shader and reads back the output buffer.
Designed for the test_v2.spv shader that computes 10+20=30.

Usage:
    python run_spirv.py [path/to/shader.spv]
"""

import sys
import struct
import ctypes
from pathlib import Path

try:
    import vulkan as vk
except ImportError:
    print("ERROR: vulkan package not installed. Run: pip install vulkan")
    sys.exit(1)


def find_suitable_physical_device(instance):
    """Find a physical device that supports compute queues."""
    devices = vk.vkEnumeratePhysicalDevices(instance)
    if not devices:
        raise RuntimeError("No Vulkan physical devices found")

    for device in devices:
        props = vk.vkGetPhysicalDeviceProperties(device)
        queue_props = vk.vkGetPhysicalDeviceQueueFamilyProperties(device)

        # Find a queue family that supports compute
        for i, queue_prop in enumerate(queue_props):
            if queue_prop.queueFlags & vk.VK_QUEUE_COMPUTE_BIT:
                device_name = props.deviceName if isinstance(props.deviceName, str) else props.deviceName.decode()
                print(f"Found device: {device_name}")
                print(f"  Queue family {i} supports compute")
                return device, i

    raise RuntimeError("No device with compute support found")


def create_logical_device(physical_device, compute_queue_family):
    """Create a logical device with compute queue."""
    queue_priority = 1.0
    queue_create_info = vk.VkDeviceQueueCreateInfo(
        sType=vk.VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO,
        queueFamilyIndex=compute_queue_family,
        queueCount=1,
        pQueuePriorities=[queue_priority]
    )

    device_create_info = vk.VkDeviceCreateInfo(
        sType=vk.VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO,
        queueCreateInfoCount=1,
        pQueueCreateInfos=[queue_create_info],
        enabledExtensionCount=0,
        ppEnabledExtensionNames=None
    )

    device = vk.vkCreateDevice(physical_device, device_create_info, None)
    queue = vk.vkGetDeviceQueue(device, compute_queue_family, 0)
    return device, queue


def create_buffer(device, physical_device, size, usage):
    """Create a buffer with device memory."""
    buffer_info = vk.VkBufferCreateInfo(
        sType=vk.VK_STRUCTURE_TYPE_BUFFER_CREATE_INFO,
        size=size,
        usage=usage,
        sharingMode=vk.VK_SHARING_MODE_EXCLUSIVE
    )

    buffer = vk.vkCreateBuffer(device, buffer_info, None)

    # Get memory requirements
    mem_reqs = vk.vkGetBufferMemoryRequirements(device, buffer)

    # Find suitable memory type
    mem_props = vk.vkGetPhysicalDeviceMemoryProperties(physical_device)

    # We need HOST_VISIBLE and HOST_COHERENT for easy readback
    required_props = vk.VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT | vk.VK_MEMORY_PROPERTY_HOST_COHERENT_BIT

    memory_type_index = None
    for i in range(mem_props.memoryTypeCount):
        if (mem_reqs.memoryTypeBits & (1 << i)) and \
           (mem_props.memoryTypes[i].propertyFlags & required_props) == required_props:
            memory_type_index = i
            break

    if memory_type_index is None:
        raise RuntimeError("Could not find suitable memory type")

    # Allocate memory
    alloc_info = vk.VkMemoryAllocateInfo(
        sType=vk.VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO,
        allocationSize=mem_reqs.size,
        memoryTypeIndex=memory_type_index
    )

    memory = vk.vkAllocateMemory(device, alloc_info, None)
    vk.vkBindBufferMemory(device, buffer, memory, 0)

    return buffer, memory


def create_descriptor_set_layout(device):
    """Create descriptor set layout for storage buffer at binding 0."""
    binding = vk.VkDescriptorSetLayoutBinding(
        binding=0,
        descriptorType=vk.VK_DESCRIPTOR_TYPE_STORAGE_BUFFER,
        descriptorCount=1,
        stageFlags=vk.VK_SHADER_STAGE_COMPUTE_BIT,
        pImmutableSamplers=None
    )

    layout_info = vk.VkDescriptorSetLayoutCreateInfo(
        sType=vk.VK_STRUCTURE_TYPE_DESCRIPTOR_SET_LAYOUT_CREATE_INFO,
        bindingCount=1,
        pBindings=[binding]
    )

    return vk.vkCreateDescriptorSetLayout(device, layout_info, None)


def create_descriptor_pool(device):
    """Create descriptor pool for storage buffer."""
    pool_size = vk.VkDescriptorPoolSize(
        type=vk.VK_DESCRIPTOR_TYPE_STORAGE_BUFFER,
        descriptorCount=1
    )

    pool_info = vk.VkDescriptorPoolCreateInfo(
        sType=vk.VK_STRUCTURE_TYPE_DESCRIPTOR_POOL_CREATE_INFO,
        maxSets=1,
        poolSizeCount=1,
        pPoolSizes=[pool_size]
    )

    return vk.vkCreateDescriptorPool(device, pool_info, None)


def create_descriptor_set(device, layout, pool, buffer, buffer_size):
    """Allocate and update descriptor set with our buffer."""
    alloc_info = vk.VkDescriptorSetAllocateInfo(
        sType=vk.VK_STRUCTURE_TYPE_DESCRIPTOR_SET_ALLOCATE_INFO,
        descriptorPool=pool,
        descriptorSetCount=1,
        pSetLayouts=[layout]
    )

    descriptor_set = vk.vkAllocateDescriptorSets(device, alloc_info)[0]

    # Update descriptor set to point to our buffer
    buffer_info = vk.VkDescriptorBufferInfo(
        buffer=buffer,
        offset=0,
        range=buffer_size
    )

    write = vk.VkWriteDescriptorSet(
        sType=vk.VK_STRUCTURE_TYPE_WRITE_DESCRIPTOR_SET,
        dstSet=descriptor_set,
        dstBinding=0,
        dstArrayElement=0,
        descriptorCount=1,
        descriptorType=vk.VK_DESCRIPTOR_TYPE_STORAGE_BUFFER,
        pBufferInfo=[buffer_info]
    )

    vk.vkUpdateDescriptorSets(device, 1, [write], 0, None)
    return descriptor_set


def create_compute_pipeline(device, layout, shader_code):
    """Create compute pipeline from SPIR-V code."""
    # Create shader module
    shader_info = vk.VkShaderModuleCreateInfo(
        sType=vk.VK_STRUCTURE_TYPE_SHADER_MODULE_CREATE_INFO,
        codeSize=len(shader_code),
        pCode=shader_code
    )

    shader_module = vk.vkCreateShaderModule(device, shader_info, None)

    # Create pipeline
    stage_info = vk.VkPipelineShaderStageCreateInfo(
        sType=vk.VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO,
        stage=vk.VK_SHADER_STAGE_COMPUTE_BIT,
        module=shader_module,
        pName="main"
    )

    pipeline_info = vk.VkComputePipelineCreateInfo(
        sType=vk.VK_STRUCTURE_TYPE_COMPUTE_PIPELINE_CREATE_INFO,
        stage=stage_info,
        layout=layout
    )

    pipeline = vk.vkCreateComputePipelines(device, None, 1, [pipeline_info], None)[0]

    # Clean up shader module (no longer needed after pipeline creation)
    vk.vkDestroyShaderModule(device, shader_module, None)

    return pipeline


def create_command_pool(device, queue_family):
    """Create command pool for compute commands."""
    pool_info = vk.VkCommandPoolCreateInfo(
        sType=vk.VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO,
        queueFamilyIndex=queue_family
    )

    return vk.vkCreateCommandPool(device, pool_info, None)


def create_command_buffer(device, command_pool, pipeline, layout, descriptor_set):
    """Create and record command buffer with compute dispatch."""
    alloc_info = vk.VkCommandBufferAllocateInfo(
        sType=vk.VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO,
        commandPool=command_pool,
        level=vk.VK_COMMAND_BUFFER_LEVEL_PRIMARY,
        commandBufferCount=1
    )

    cmd_buffer = vk.vkAllocateCommandBuffers(device, alloc_info)[0]

    # Begin recording
    begin_info = vk.VkCommandBufferBeginInfo(
        sType=vk.VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO,
        flags=vk.VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT
    )
    vk.vkBeginCommandBuffer(cmd_buffer, begin_info)

    # Record commands
    vk.vkCmdBindPipeline(cmd_buffer, vk.VK_PIPELINE_BIND_POINT_COMPUTE, pipeline)
    vk.vkCmdBindDescriptorSets(cmd_buffer, vk.VK_PIPELINE_BIND_POINT_COMPUTE,
                               layout, 0, 1, [descriptor_set], 0, None)
    vk.vkCmdDispatch(cmd_buffer, 1, 1, 1)  # Single workgroup

    # End recording
    vk.vkEndCommandBuffer(cmd_buffer)

    return cmd_buffer


def run_compute_shader(spirv_path: str):
    """Load and execute a SPIR-V compute shader, return buffer contents."""

    # Load SPIR-V binary
    spirv_path = Path(spirv_path)
    if not spirv_path.exists():
        raise FileNotFoundError(f"SPIR-V file not found: {spirv_path}")

    with open(spirv_path, "rb") as f:
        shader_code = f.read()

    print(f"Loaded SPIR-V: {spirv_path} ({len(shader_code)} bytes)")

    # Create Vulkan instance
    app_info = vk.VkApplicationInfo(
        sType=vk.VK_STRUCTURE_TYPE_APPLICATION_INFO,
        pApplicationName="Geometry OS SPIR-V Runner",
        applicationVersion=1,
        pEngineName="Geometry OS",
        engineVersion=1,
        apiVersion=vk.VK_API_VERSION_1_0
    )

    instance_info = vk.VkInstanceCreateInfo(
        sType=vk.VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO,
        pApplicationInfo=app_info
    )

    instance = vk.vkCreateInstance(instance_info, None)
    print("Created Vulkan instance")

    try:
        # Find physical device
        physical_device, compute_queue_family = find_suitable_physical_device(instance)

        # Create logical device and queue
        device, queue = create_logical_device(physical_device, compute_queue_family)
        print("Created logical device")

        try:
            # Create output buffer (16 bytes for our float struct)
            buffer_size = 16
            buffer, buffer_memory = create_buffer(
                device, physical_device, buffer_size,
                vk.VK_BUFFER_USAGE_STORAGE_BUFFER_BIT
            )
            print(f"Created output buffer ({buffer_size} bytes)")

            # Create descriptor set layout
            ds_layout = create_descriptor_set_layout(device)

            # Create pipeline layout
            pipeline_layout_info = vk.VkPipelineLayoutCreateInfo(
                sType=vk.VK_STRUCTURE_TYPE_PIPELINE_LAYOUT_CREATE_INFO,
                setLayoutCount=1,
                pSetLayouts=[ds_layout]
            )
            pipeline_layout = vk.vkCreatePipelineLayout(device, pipeline_layout_info, None)

            # Create descriptor pool and set
            descriptor_pool = create_descriptor_pool(device)
            descriptor_set = create_descriptor_set(device, ds_layout, descriptor_pool, buffer, buffer_size)

            # Create compute pipeline
            pipeline = create_compute_pipeline(device, pipeline_layout, shader_code)
            print("Created compute pipeline")

            # Create command pool and buffer
            command_pool = create_command_pool(device, compute_queue_family)
            command_buffer = create_command_buffer(
                device, command_pool, pipeline, pipeline_layout, descriptor_set
            )
            print("Recorded command buffer")

            # Create fence for synchronization
            fence_info = vk.VkFenceCreateInfo(
                sType=vk.VK_STRUCTURE_TYPE_FENCE_CREATE_INFO
            )
            fence = vk.vkCreateFence(device, fence_info, None)

            # Submit and wait
            submit_info = vk.VkSubmitInfo(
                sType=vk.VK_STRUCTURE_TYPE_SUBMIT_INFO,
                commandBufferCount=1,
                pCommandBuffers=[command_buffer]
            )

            vk.vkQueueSubmit(queue, 1, [submit_info], fence)
            print("Submitted compute shader")

            # Wait for completion
            vk.vkWaitForFences(device, 1, [fence], vk.VK_TRUE, 5_000_000_000)  # 5 second timeout
            print("Compute shader completed")

            # Read back buffer
            # Map memory - vulkan package returns a ffi.buffer object
            buffer_data = vk.vkMapMemory(device, buffer_memory, 0, buffer_size, 0)

            # Read first float from buffer (struct format: '<f' = little-endian float)
            result = struct.unpack('<f', buffer_data[:4])[0]
            vk.vkUnmapMemory(device, buffer_memory)

            print(f"\n=== RESULT ===")
            print(f"Output buffer value: {result}")

            # Cleanup
            vk.vkDestroyFence(device, fence, None)
            vk.vkDestroyCommandPool(device, command_pool, None)
            vk.vkDestroyPipeline(device, pipeline, None)
            vk.vkDestroyDescriptorPool(device, descriptor_pool, None)
            vk.vkDestroyPipelineLayout(device, pipeline_layout, None)
            vk.vkDestroyDescriptorSetLayout(device, ds_layout, None)
            vk.vkDestroyBuffer(device, buffer, None)
            vk.vkFreeMemory(device, buffer_memory, None)

            return result

        finally:
            vk.vkDestroyDevice(device, None)

    finally:
        vk.vkDestroyInstance(instance, None)


if __name__ == "__main__":
    # Default to test_v2.spv in current directory
    spirv_file = sys.argv[1] if len(sys.argv) > 1 else "test_v2.spv"

    print(f"Geometry OS SPIR-V Runner")
    print(f"=========================")
    print(f"Running: {spirv_file}\n")

    result = run_compute_shader(spirv_file)

    print(f"\nExpected: 30.0 (10.0 + 20.0)")
    print(f"Got:      {result}")

    if abs(result - 30.0) < 0.001:
        print("\n✓ SUCCESS: Compute shader executed correctly!")
        sys.exit(0)
    else:
        print("\n✗ FAILURE: Result mismatch!")
        sys.exit(1)
